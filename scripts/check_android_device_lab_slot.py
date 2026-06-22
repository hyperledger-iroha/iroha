"""Validate Android device-lab slots for AND6 compliance evidence."""

from __future__ import annotations

import argparse
from collections.abc import Mapping
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Iterable
import unicodedata


EXPECTED_DIRS: tuple[str, ...] = ("telemetry", "attestation", "queue", "logs")
OPTIONAL_EVIDENCE_DIRS: tuple[str, ...] = ("evidence", "handoff", "wallet")
REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS: tuple[str, ...] = (
    "telemetry/telemetry.json",
    "telemetry/status.ndjson",
    "attestation/harness-result.json",
    "attestation/result.json",
    "attestation/report.json",
    "queue/pending_queue.json",
    "logs/runtime.log",
)
KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH = "evidence/signed-evidence.json"
MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES = 64 * 1024 * 1024
MAX_ANDROID_DEVICE_LAB_JSON_BYTES = 16 * 1024 * 1024
KAGEMUSHA_OFFLINE_WALLET_APK_PATH = "evidence/offline-wallet-release.apk"
MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES = 1024 * 1024
KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER = "kagemusha device-lab run complete"
KAGEMUSHA_TELEMETRY_SUITE = "kagemusha-device-lab"
KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS: tuple[str, ...] = (
    "BUILD FAILED",
    "TEST FAILED",
    "FATAL EXCEPTION",
    "Traceback",
    "panicked at",
)
KAGEMUSHA_STATUS_FAILURE_VALUES = {
    "failed",
    "failure",
    "error",
    "panic",
    "cancelled",
    "timeout",
    "timed_out",
}
STATUS_EVENT_FIELDS: frozenset[str] = frozenset(
    {
        "status",
        "slot_id",
    }
)


def _slot_artifact_max_bytes(relative: str) -> int:
    if relative == KAGEMUSHA_OFFLINE_WALLET_APK_PATH:
        return MAX_KAGEMUSHA_OFFLINE_WALLET_APK_BYTES
    return MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
SUMMARY_REDACTION_KEY_COLLISION_FIELD = "summary_redaction_key_collision"
SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD = "summary_non_string_key_normalized"
SUMMARY_NON_STRING_KEY_REDACTION = "<non-string-summary-key>"
SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD = "summary_nonfinite_number_normalized"
SUMMARY_NONFINITE_NUMBER_REDACTION = "<non-finite-summary-number>"
SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD = "summary_unsupported_value_normalized"
SUMMARY_UNSUPPORTED_VALUE_REDACTION = "<unsupported-summary-value>"
SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD = "summary_kagemusha_shape_normalized"
SUMMARY_STATUS_NORMALIZED_FIELD = "summary_status_normalized"
SUMMARY_ERRORS_NORMALIZED_FIELD = "summary_errors_normalized"
SUMMARY_ERROR_REDACTION = "<malformed-summary-error>"
SHA256_HEX_RE = re.compile(r"^[0-9a-f]{64}$")
SIGNED_AT_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
SECRET_RE = re.compile(
    r"(authorization:|bearer\s+|private[_-]?key|token=|x-iroha-signature)",
    re.IGNORECASE,
)
PRIVATE_KEY_PEM_MARKERS = (
    b"-----BEGIN PRIVATE KEY-----",
    b"-----BEGIN ENCRYPTED PRIVATE KEY-----",
    b"-----BEGIN RSA PRIVATE KEY-----",
    b"-----BEGIN EC PRIVATE KEY-----",
    b"-----BEGIN DSA PRIVATE KEY-----",
    b"-----BEGIN OPENSSH PRIVATE KEY-----",
)
KAGEMUSHA_STANDARD_DEVICE_FAMILIES: tuple[str, ...] = (
    "Google Pixel 6 / 6a",
    "Google Pixel 7 / 7 Pro",
    "Google Pixel 8 / 8a / 8 Pro",
    "Google Pixel Fold / Tablet",
    "Samsung Galaxy S23",
    "Samsung Galaxy S24",
)
KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS: dict[str, str] = {
    "Google Pixel 6 / 6a": "Android 14",
    "Google Pixel 7 / 7 Pro": "Android 14",
    "Google Pixel 8 / 8a / 8 Pro": "Android 15",
    "Google Pixel Fold / Tablet": "Android 15",
    "Samsung Galaxy S23": "Android 14",
    "Samsung Galaxy S24": "Android 15",
}
KAGEMUSHA_DEVICE_FAMILY_MODEL_RULES: tuple[
    tuple[str, tuple[str, ...], tuple[str, ...], tuple[str, ...]], ...
] = (
    ("Google Pixel 6 / 6a", ("pixel 6", "pixel 6a"), ("oriole", "bluejay"), ()),
    ("Google Pixel 7 / 7 Pro", ("pixel 7", "pixel 7 pro"), ("panther", "cheetah"), ()),
    (
        "Google Pixel 8 / 8a / 8 Pro",
        ("pixel 8", "pixel 8a", "pixel 8 pro"),
        ("shiba", "akita", "husky"),
        (),
    ),
    (
        "Google Pixel Fold / Tablet",
        ("pixel fold", "pixel tablet"),
        ("felix", "tangorpro"),
        (),
    ),
    (
        "Samsung Galaxy S23",
        (),
        ("dm1q", "dm2q", "dm3q"),
        ("sm-s911", "sm-s916", "sm-s918"),
    ),
    (
        "Samsung Galaxy S24",
        (),
        ("e1q", "e2q", "e3q"),
        ("sm-s921", "sm-s926", "sm-s928"),
    ),
)
RAW_TEST_COMMAND_REQUIRED_MARKERS: tuple[str, ...] = (
    ":client-android:assembleRelease",
    ":offline-wallet-android:assembleRelease",
    ":offline-wallet-android:connectedDebugAndroidTest",
    ":offline-wallet-lab-app:assembleRelease",
    ":offline-wallet-lab-app:installRelease",
    ":offline-wallet-lab-app:installReleaseAndroidTest",
    "adb shell am instrument",
    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest",
    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest",
    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest",
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND = (
    "ANDROID_HARNESS_MAINS="
    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest "
    "./gradlew :client-android:assembleRelease "
    ":offline-wallet-android:assembleRelease "
    ":offline-wallet-android:connectedDebugAndroidTest "
    "-Pandroid.testInstrumentationRunnerArguments.class="
    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest,"
    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND = (
    "./gradlew :offline-wallet-lab-app:assembleRelease "
    ":offline-wallet-lab-app:installRelease "
    ":offline-wallet-lab-app:installReleaseAndroidTest"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_INSTRUMENT_COMMAND = (
    "adb shell am instrument -w -e class "
    "org.hyperledger.iroha.android.offline.KagemushaDeviceLabArtifactExportTest "
    "org.hyperledger.iroha.sdk.offline.wallet.lab.test/"
    "androidx.test.runner.AndroidJUnitRunner"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS: tuple[str, ...] = (
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_INSTRUMENT_COMMAND,
)
SIGNED_EVIDENCE_SCHEMA = "iroha.android.device_lab.kagemusha.signed_evidence.v1"
D2D_PAYMENT_TRANSCRIPT_SCHEMA = "iroha.android.device_lab.kagemusha.d2d_payment.v1"
D2D_PAYMENT_PAYLOAD_SCHEMA = "kagemusha.recursive_spend.reserved_lineage.d2d.v1"
WALLET_INTEGRITY_TRANSCRIPT_SCHEMA = (
    "iroha.android.device_lab.kagemusha.wallet_integrity.v1"
)
D2D_PAYMENT_TRANSPORTS = {"nearby_offline", "nfc_hce", "qr"}
D2D_PAYMENT_TRANSCRIPTS_FIELD = "d2d_payment_transcripts"
D2D_PAYMENT_TRANSCRIPT_ENTRY_FIELDS = frozenset({"path", "sha256"})
MAX_D2D_PAYMENT_PAYLOAD_BYTES = 16 * 1024
ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES = (".der", ".pem")
MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES = 64 * 1024
SIGNED_EVIDENCE_SIGNATURE_ALGORITHMS = {"ed25519"}
ED25519_SIGNATURE_BYTES = 64
REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION = 7
ABI7_RECURSIVE_COMPACT_ONE_HOP_JNI_PROBE_STATES = {"one_hop_verified"}
ABI7_RECURSIVE_COMPACT_MULTI_HOP_PROVER_STATES = {"multi_hop_proof_composed"}
SIGNED_EVIDENCE_SLOT_STRING_FIELDS: tuple[str, ...] = (
    "slot_id",
    "device_family",
    "device_model",
    "device_codename",
    "device_fingerprint",
    "os_build_id",
    "minimum_os",
    "app_package_name",
    "attestation_certificate_chain_path",
    "offline_wallet_apk_path",
    "d2d_payment_transcript_path",
    "wallet_integrity_transcript_path",
    "keymint_security_level",
    "abi6_recursive_spend_jni_probe",
    "abi7_recursive_compact_jni_probe",
    "abi7_recursive_compact_prover_state",
)
SIGNED_EVIDENCE_SLOT_ARTIFACT_PATH_FIELDS: tuple[str, ...] = (
    "attestation_certificate_chain_path",
    "offline_wallet_apk_path",
    "d2d_payment_transcript_path",
    "wallet_integrity_transcript_path",
)
SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple[str, ...] = (
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "attestation_certificate_chain_sha256",
    "offline_wallet_policy_sha256",
    "offline_wallet_apk_sha256",
    "d2d_payment_transcript_sha256",
    "wallet_integrity_transcript_sha256",
)
SIGNED_EVIDENCE_SLOT_INT_FIELDS: tuple[str, ...] = (
    "native_bridge_abi_version",
)
SIGNED_EVIDENCE_SLOT_TRUE_FIELDS: tuple[str, ...] = (
    "strongbox_attestation",
    "physical_device_attestation",
    "one_use_key_rotation_passed",
    "rollback_rejection_passed",
)
PENDING_QUEUE_FIELDS: frozenset[str] = frozenset(
    {
        "slot_id",
        "pending_transactions",
    }
)
TELEMETRY_FIELDS: frozenset[str] = frozenset(
    {
        "schema_version",
        "slot_id",
        "suite",
        "device_model",
        "device_codename",
        "app_package_name",
    }
)
TELEMETRY_STRING_FIELDS: tuple[str, ...] = (
    "device_model",
    "device_codename",
    "app_package_name",
)
SLOT_METADATA_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *SIGNED_EVIDENCE_SLOT_STRING_FIELDS,
        *SIGNED_EVIDENCE_SLOT_SHA256_FIELDS,
        *SIGNED_EVIDENCE_SLOT_INT_FIELDS,
        *SIGNED_EVIDENCE_SLOT_TRUE_FIELDS,
        "raw_test_commands",
        D2D_PAYMENT_TRANSCRIPTS_FIELD,
        "signed_evidence_artifact_path",
        "signed_evidence_artifact_sha256",
    }
)
SIGNED_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "slot_id",
        "device_family",
        "device_model",
        "device_codename",
        "device_fingerprint",
        "os_build_id",
        "minimum_os",
        "app_package_name",
        "attestation_certificate_chain_path",
        "offline_wallet_apk_path",
        "d2d_payment_transcript_path",
        "wallet_integrity_transcript_path",
        "app_signing_certificate_sha256",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_sha256",
        "offline_wallet_policy_sha256",
        "offline_wallet_apk_sha256",
        "d2d_payment_transcript_sha256",
        "wallet_integrity_transcript_sha256",
        "native_bridge_abi_version",
        "strongbox_attestation",
        "physical_device_attestation",
        "keymint_security_level",
        "one_use_key_rotation_passed",
        "rollback_rejection_passed",
        "abi6_recursive_spend_jni_probe",
        "abi7_recursive_compact_jni_probe",
        "abi7_recursive_compact_prover_state",
        D2D_PAYMENT_TRANSCRIPTS_FIELD,
        "raw_test_commands",
        "signed_at_utc",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
        "artifact_digests",
    }
)
ATTESTATION_RESULT_SLOT_BINDING_FIELDS: tuple[str, ...] = (
    "device_fingerprint",
    "os_build_id",
    "app_package_name",
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "attestation_certificate_chain_path",
    "attestation_certificate_chain_sha256",
    "offline_wallet_policy_sha256",
)
ATTESTATION_RESULT_FIELDS: frozenset[str] = frozenset(
    {
        "slot",
        "slot_id",
        "status",
        *ATTESTATION_RESULT_SLOT_BINDING_FIELDS,
        "attestation_security_level",
        "keymaster_security_level",
        "keymint_security_level",
        "strongbox_attestation",
        "physical_device_attestation",
    }
)
ATTESTATION_REPORT_SCHEMA = "iroha.android.device_lab.kagemusha.attestation_report.v1"
ATTESTATION_REPORT_SLOT_BINDING_FIELDS: tuple[str, ...] = (
    "slot_id",
    "device_fingerprint",
    "os_build_id",
    "app_package_name",
    "attestation_challenge_sha256",
    "attestation_certificate_chain_path",
    "attestation_certificate_chain_sha256",
)
ATTESTATION_REPORT_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *ATTESTATION_REPORT_SLOT_BINDING_FIELDS,
        "verifier",
        "verification",
    }
)
ATTESTATION_REPORT_VERIFICATION_FIELDS: frozenset[str] = frozenset(
    {
        "status",
        "strongbox_attestation",
        "physical_device_attestation",
        "keymint_security_level",
        "attestation_security_level",
        "keymaster_security_level",
    }
)
ATTESTATION_HARNESS_RESULT_FIELDS: frozenset[str] = frozenset(
    {
        "alias",
        "attestation_security_level",
        "keymaster_security_level",
        "strongbox_attestation",
        "challenge_hex",
        "chain_length",
    }
)
D2D_PAYMENT_TRANSCRIPT_SLOT_STRING_BINDINGS: tuple[str, ...] = (
    "slot_id",
    "device_family",
    "device_fingerprint",
    "os_build_id",
    "app_package_name",
)
D2D_PAYMENT_TRANSCRIPT_SLOT_SHA256_BINDINGS: tuple[str, ...] = (
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "offline_wallet_policy_sha256",
    "offline_wallet_apk_sha256",
)
D2D_PAYMENT_TRANSCRIPT_SHA256_FIELDS: tuple[str, ...] = (
    *D2D_PAYMENT_TRANSCRIPT_SLOT_SHA256_BINDINGS,
    "transport_session_id_sha256",
    "payload_sha256",
    "received_payload_sha256",
    "receiver_ack_sha256",
    "one_use_key_id_sha256",
    "payer_wallet_state_before_sha256",
    "payer_wallet_state_after_sha256",
    "payee_wallet_state_before_sha256",
    "payee_wallet_state_after_sha256",
    "queue_before_sha256",
    "queue_after_sha256",
)
D2D_PAYMENT_TRANSCRIPT_TRUE_FIELDS: tuple[str, ...] = (
    "transport_offline",
    "payer_wallet_offline",
    "payee_wallet_offline",
    "one_use_key_consumed",
    "receiver_redeem_accepted",
    "double_spend_rejected",
)
D2D_PAYMENT_TRANSCRIPT_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *D2D_PAYMENT_TRANSCRIPT_SLOT_STRING_BINDINGS,
        *D2D_PAYMENT_TRANSCRIPT_SHA256_FIELDS,
        *D2D_PAYMENT_TRANSCRIPT_TRUE_FIELDS,
        "transport",
        "payload_schema",
        "payload_bytes",
    }
)
WALLET_INTEGRITY_TRANSCRIPT_SLOT_STRING_BINDINGS: tuple[str, ...] = (
    "slot_id",
    "device_family",
    "device_fingerprint",
    "os_build_id",
    "app_package_name",
    "keymint_security_level",
)
WALLET_INTEGRITY_TRANSCRIPT_SLOT_SHA256_BINDINGS: tuple[str, ...] = (
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "attestation_certificate_chain_sha256",
    "offline_wallet_policy_sha256",
    "offline_wallet_apk_sha256",
)
WALLET_INTEGRITY_TRANSCRIPT_SHA256_FIELDS: tuple[str, ...] = (
    *WALLET_INTEGRITY_TRANSCRIPT_SLOT_SHA256_BINDINGS,
    "rotation_session_id_sha256",
    "key_id_before_sha256",
    "key_id_after_sha256",
    "wallet_state_before_sha256",
    "wallet_state_after_rotation_sha256",
    "rollback_snapshot_sha256",
    "restored_snapshot_sha256",
)
WALLET_INTEGRITY_TRANSCRIPT_TRUE_FIELDS: tuple[str, ...] = (
    "one_use_key_rotation_passed",
    "old_key_invalidated",
    "rollback_rejection_passed",
    "stale_snapshot_rejected",
    "active_wallet_state_preserved_after_reject",
)
WALLET_INTEGRITY_TRANSCRIPT_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *WALLET_INTEGRITY_TRANSCRIPT_SLOT_STRING_BINDINGS,
        *WALLET_INTEGRITY_TRANSCRIPT_SHA256_FIELDS,
        *WALLET_INTEGRITY_TRANSCRIPT_TRUE_FIELDS,
    }
)
STRONGBOX_LEVELS = {"STRONGBOX", "STRONG_BOX"}
SECRET_PATH_REDACTION = "<redacted-secret-path>"
CONTROL_PATH_REDACTION = "<unsafe-path>"


def _slot_root_entries(slot_path: Path, errors: list[str]) -> list[Path] | None:
    try:
        return sorted(slot_path.iterdir(), key=lambda entry: entry.name)
    except OSError:
        _append_error_once(errors, "slot directory could not be listed")
        return None


def _record_manifest_inventory_entry(
    slot_path: Path,
    entry: Path,
    files: set[str],
    errors: list[str],
) -> None:
    relative = entry.relative_to(slot_path).as_posix()
    try:
        mode = entry.lstat().st_mode
    except OSError:
        _append_error_once(
            errors,
            f"slot artifact {_display_path(relative)} file metadata could not be read",
        )
        return
    if stat.S_ISREG(mode) or stat.S_ISLNK(mode):
        files.add(relative)


def _slot_files(slot_path: Path, errors: list[str] | None = None) -> set[str]:
    slot_errors = errors if errors is not None else []
    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        slot_errors.extend(path_errors)
        return set()
    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        return set()
    except OSError:
        _append_error_once(slot_errors, "slot directory metadata could not be read")
        return set()
    if stat.S_ISLNK(slot_mode) or not stat.S_ISDIR(slot_mode):
        return set()
    if validate_no_symlink_ancestors(slot_path, "slot ancestor directory"):
        return set()
    files: set[str] = set()
    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        try:
            dir_mode = dir_path.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            _append_error_once(slot_errors, f"{dirname}/ metadata could not be read")
            continue
        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):
            continue
        entries = _slot_tree_entries(dir_path, f"{dirname}/", slot_errors)
        if entries is None:
            continue
        for entry in entries:
            _record_manifest_inventory_entry(slot_path, entry, files, slot_errors)
    skipped_roots = {"sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    root_entries = _slot_root_entries(slot_path, slot_errors)
    if root_entries is None:
        return files
    for entry in root_entries:
        if entry.name in skipped_roots:
            continue
        _record_manifest_inventory_entry(slot_path, entry, files, slot_errors)
    return files


def _slot_relative_symlink_ancestor(slot_path: Path, relative: str) -> str | None:
    current = slot_path
    for part in PurePosixPath(relative).parts[:-1]:
        current = current / part
        try:
            current_mode = current.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            return current.relative_to(slot_path).as_posix()
        if stat.S_ISLNK(current_mode):
            return current.relative_to(slot_path).as_posix()
    return None


def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:
    """Reject symlinked slot metadata, directories, and evidence artifacts."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in ("slot.json", "sha256sum.txt"):
        path = slot_path / relative
        try:
            mode = path.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            _append_error_once(errors, f"{relative} file metadata could not be read")
            continue
        if stat.S_ISLNK(mode):
            errors.append(f"{relative} must not be a symlink")

    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        try:
            dir_mode = dir_path.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            _append_error_once(errors, f"{dirname}/ metadata could not be read")
            continue
        if stat.S_ISLNK(dir_mode):
            errors.append(f"{dirname}/ must not be a symlink")
            continue
        if not stat.S_ISDIR(dir_mode):
            continue
        entries = _slot_tree_entries(dir_path, f"{dirname}/", errors)
        if entries is None:
            continue
        for entry in entries:
            relative = entry.relative_to(slot_path).as_posix()
            try:
                entry_mode = entry.lstat().st_mode
            except OSError:
                _append_error_once(
                    errors,
                    f"slot artifact {_display_path(relative)} file metadata could not be read",
                )
                continue
            if stat.S_ISLNK(entry_mode):
                errors.append(
                    f"slot artifact {_display_path(relative)} must not be a symlink"
                )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    root_entries = _slot_root_entries(slot_path, errors)
    if root_entries is None:
        return
    for entry in root_entries:
        if entry.name in skipped_roots:
            continue
        relative = entry.relative_to(slot_path).as_posix()
        try:
            entry_mode = entry.lstat().st_mode
        except OSError:
            _append_error_once(
                errors,
                f"slot artifact {_display_path(relative)} file metadata could not be read",
            )
            continue
        if stat.S_ISLNK(entry_mode):
            errors.append(f"slot artifact {_display_path(relative)} must not be a symlink")


def _reject_hardlinked_file(path: Path, label: str, errors: list[str]) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return
    if stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
        return
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")


def validate_no_slot_hardlink_artifacts(slot_path: Path, errors: list[str]) -> None:
    """Reject hardlinked slot metadata and evidence artifacts."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in ("slot.json", "sha256sum.txt"):
        _reject_hardlinked_file(slot_path / relative, relative, errors)

    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        try:
            dir_mode = dir_path.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            _append_error_once(errors, f"{dirname}/ metadata could not be read")
            continue
        if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):
            continue
        entries = _slot_tree_entries(dir_path, f"{dirname}/", errors)
        if entries is None:
            continue
        for entry in entries:
            relative = entry.relative_to(slot_path).as_posix()
            _reject_hardlinked_file(
                entry,
                f"slot artifact {_display_path(relative)}",
                errors,
            )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    root_entries = _slot_root_entries(slot_path, errors)
    if root_entries is None:
        return
    for entry in root_entries:
        if entry.name in skipped_roots:
            continue
        relative = entry.relative_to(slot_path).as_posix()
        _reject_hardlinked_file(
            entry,
            f"slot artifact {_display_path(relative)}",
            errors,
        )


def _reject_non_regular_file(path: Path, label: str, errors: list[str]) -> None:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return
    if stat.S_ISLNK(mode):
        return
    if not stat.S_ISREG(mode):
        errors.append(f"{label} must be a regular file")


def validate_slot_regular_file_artifacts(slot_path: Path, errors: list[str]) -> None:
    """Reject special-file slot metadata and evidence artifacts."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in ("slot.json", "sha256sum.txt"):
        _reject_non_regular_file(slot_path / relative, relative, errors)

    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        try:
            mode = dir_path.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            errors.append(f"{dirname}/ metadata could not be read")
            continue
        if stat.S_ISLNK(mode):
            continue
        if not stat.S_ISDIR(mode):
            errors.append(f"{dirname}/ must be a directory")
            continue
        entries = _slot_tree_entries(dir_path, f"{dirname}/", errors)
        if entries is None:
            continue
        for entry in entries:
            relative = entry.relative_to(slot_path).as_posix()
            try:
                entry_mode = entry.lstat().st_mode
            except OSError:
                errors.append(
                    f"slot artifact {_display_path(relative)} file metadata could not be read"
                )
                continue
            if stat.S_ISLNK(entry_mode):
                continue
            if stat.S_ISDIR(entry_mode):
                continue
            if not stat.S_ISREG(entry_mode):
                errors.append(
                    f"slot artifact {_display_path(relative)} must be a regular file"
                )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    root_entries = _slot_root_entries(slot_path, errors)
    if root_entries is None:
        return
    for entry in root_entries:
        if entry.name in skipped_roots:
            continue
        _reject_non_regular_file(
            entry,
            f"slot artifact {_display_path(entry.relative_to(slot_path).as_posix())}",
            errors,
        )


def _normalise_safe_relative_path(
    path_text: str,
    errors: list[str],
    label: str,
    *,
    allow_sha_manifest: bool = False,
) -> str | None:
    if _contains_control_character(path_text):
        errors.append(f"{label}: unsafe path contains control characters")
        return None
    candidate = PurePosixPath(path_text)
    if path_text != path_text.strip() or any(
        part != part.strip() for part in candidate.parts
    ):
        errors.append(f"{label}: unsafe path contains surrounding whitespace")
        return None
    if SECRET_RE.search(path_text):
        errors.append(f"{label}: unsafe path contains secret-looking material")
        return None
    if (
        not path_text
        or path_text.startswith("*")
        or path_text.startswith("/")
        or "\\" in path_text
        or candidate.is_absolute()
        or ".." in candidate.parts
        or candidate.as_posix() in {"", "."}
        or (not allow_sha_manifest and candidate.as_posix() == "sha256sum.txt")
    ):
        errors.append(f"{label}: unsafe path {_display_path(path_text)!r}")
        return None
    if candidate.as_posix() != path_text:
        errors.append(f"{label}: unsafe path is not canonical")
        return None
    return candidate.as_posix()


def _safe_relative_path_is_child_of(path_text: str, root: str) -> bool:
    prefix = f"{root}/"
    return path_text.startswith(prefix) and len(path_text) > len(prefix)


def _display_path(path_text: str) -> str:
    if SECRET_RE.search(path_text):
        return SECRET_PATH_REDACTION
    if _contains_control_character(path_text):
        return CONTROL_PATH_REDACTION
    return path_text


def _contains_control_character(value: str) -> bool:
    """Return whether a filesystem label carries non-printing control text."""

    return any(
        ord(character) < 0x20
        or ord(character) == 0x7F
        or unicodedata.category(character) == "Cf"
        for character in value
    )


def _display_slot_name(slot_name: str) -> str:
    """Render a slot name in diagnostics without leaking unsafe terminal controls."""

    if SECRET_RE.search(slot_name):
        return SECRET_PATH_REDACTION
    if _contains_control_character(slot_name):
        return "<unsafe-slot-name>"
    return slot_name


def _summary_safe_string(value: str) -> str:
    """Render scanner summary strings without echoing secrets or controls."""

    if SECRET_RE.search(value):
        return SECRET_PATH_REDACTION
    if _contains_control_character(value):
        return CONTROL_PATH_REDACTION
    return value


def _summary_safe_value(value: Any) -> tuple[Any, bool, bool, bool, bool]:
    """Return a JSON-summary-safe copy plus collision/normalization flags."""

    if isinstance(value, str):
        return _summary_safe_string(value), False, False, False, False
    if value is None or isinstance(value, (bool, int)):
        return value, False, False, False, False
    if isinstance(value, float) and not math.isfinite(value):
        return SUMMARY_NONFINITE_NUMBER_REDACTION, False, False, True, False
    if isinstance(value, float):
        return SUMMARY_UNSUPPORTED_VALUE_REDACTION, False, False, False, True
    if isinstance(value, list):
        items = []
        collision = False
        key_normalized = False
        value_normalized = False
        unsupported_normalized = False
        for item in value:
            (
                safe_item,
                item_collision,
                item_key_normalized,
                item_value_normalized,
                item_unsupported_normalized,
            ) = _summary_safe_value(item)
            items.append(safe_item)
            collision = collision or item_collision
            key_normalized = key_normalized or item_key_normalized
            value_normalized = value_normalized or item_value_normalized
            unsupported_normalized = (
                unsupported_normalized or item_unsupported_normalized
            )
        return items, collision, key_normalized, value_normalized, unsupported_normalized
    if isinstance(value, tuple):
        items = []
        collision = False
        key_normalized = False
        value_normalized = False
        unsupported_normalized = False
        for item in value:
            (
                safe_item,
                item_collision,
                item_key_normalized,
                item_value_normalized,
                item_unsupported_normalized,
            ) = _summary_safe_value(item)
            items.append(safe_item)
            collision = collision or item_collision
            key_normalized = key_normalized or item_key_normalized
            value_normalized = value_normalized or item_value_normalized
            unsupported_normalized = (
                unsupported_normalized or item_unsupported_normalized
            )
        return items, collision, key_normalized, value_normalized, unsupported_normalized
    if isinstance(value, dict):
        safe: dict[Any, Any] = {}
        collision = False
        key_normalized = False
        value_normalized = False
        unsupported_normalized = False
        for key, item in value.items():
            if isinstance(key, str):
                safe_key = _summary_safe_string(key)
            else:
                safe_key = SUMMARY_NON_STRING_KEY_REDACTION
                key_normalized = True
            (
                safe_item,
                item_collision,
                item_key_normalized,
                item_value_normalized,
                item_unsupported_normalized,
            ) = _summary_safe_value(item)
            if safe_key in safe:
                collision = True
                continue
            safe[safe_key] = safe_item
            collision = collision or item_collision
            key_normalized = key_normalized or item_key_normalized
            value_normalized = value_normalized or item_value_normalized
            unsupported_normalized = (
                unsupported_normalized or item_unsupported_normalized
            )
        return safe, collision, key_normalized, value_normalized, unsupported_normalized
    return SUMMARY_UNSUPPORTED_VALUE_REDACTION, False, False, False, True


def _summary_safe_report(report: dict) -> dict:
    """Return a summary-facing copy of a slot report."""

    (
        summary_report,
        key_collision,
        key_normalized,
        value_normalized,
        unsupported_normalized,
    ) = (
        _summary_safe_value(report)
    )
    if not isinstance(summary_report, dict):
        return {"status": "error", SUMMARY_STATUS_NORMALIZED_FIELD: True}
    slot = report.get("slot")
    if isinstance(slot, str):
        summary_report["slot"] = _display_slot_name(slot)
    if summary_report.get("status") not in {"ok", "error"}:
        summary_report["status"] = "error"
        summary_report[SUMMARY_STATUS_NORMALIZED_FIELD] = True
    if "kagemusha" in summary_report and not isinstance(
        summary_report["kagemusha"], dict
    ):
        summary_report["kagemusha"] = {}
        summary_report[SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD] = True
    if "errors" in summary_report:
        errors, errors_normalized = _summary_safe_errors(summary_report["errors"])
        summary_report["errors"] = errors
        if errors_normalized:
            summary_report[SUMMARY_ERRORS_NORMALIZED_FIELD] = True
    if key_collision:
        summary_report[SUMMARY_REDACTION_KEY_COLLISION_FIELD] = True
    if key_normalized:
        summary_report[SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD] = True
    if value_normalized:
        summary_report[SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD] = True
    if unsupported_normalized:
        summary_report[SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD] = True
    return summary_report


def _summary_safe_errors(value: Any) -> tuple[list[str], bool]:
    """Return a scanner-summary-safe list of error strings."""

    if not isinstance(value, list):
        return [SUMMARY_ERROR_REDACTION], True
    errors: list[str] = []
    normalized = False
    for item in value:
        if isinstance(item, str):
            errors.append(_summary_safe_string(item))
        else:
            errors.append(SUMMARY_ERROR_REDACTION)
            normalized = True
    return errors, normalized


def _summary_kagemusha(report: dict) -> dict[str, Any]:
    """Return summary-facing Kagemusha details only when they are shaped as an object."""

    kagemusha = report.get("kagemusha")
    return kagemusha if isinstance(kagemusha, dict) else {}


def _summary_device_family(report: dict) -> str | None:
    """Return a canonical Kagemusha device family from a summary report."""

    family = _summary_kagemusha(report).get("device_family")
    if isinstance(family, str) and family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES:
        return family
    return None


KAGEMUSHA_SUMMARY_RELEASE_ARTIFACTS: tuple[tuple[str, str], ...] = (
    (
        "attestation_certificate_chain_path",
        "attestation_certificate_chain_sha256",
    ),
    ("offline_wallet_apk_path", "offline_wallet_apk_sha256"),
    ("d2d_payment_transcript_path", "d2d_payment_transcript_sha256"),
    ("wallet_integrity_transcript_path", "wallet_integrity_transcript_sha256"),
)
KAGEMUSHA_SUMMARY_RELEASE_ARTIFACT_ROOTS: dict[str, str] = {
    "attestation_certificate_chain_path": "attestation",
    "offline_wallet_apk_path": "evidence",
    "d2d_payment_transcript_path": "handoff",
    "wallet_integrity_transcript_path": "wallet",
}
KAGEMUSHA_SUMMARY_RELEASE_SHA256_FIELDS: tuple[str, ...] = (
    "signed_evidence_artifact_sha256",
    "signed_evidence_signer_public_key_sha256",
    "device_fingerprint_sha256",
    "attestation_challenge_sha256",
    *(
        digest_field
        for _, digest_field in KAGEMUSHA_SUMMARY_RELEASE_ARTIFACTS
    ),
)
KAGEMUSHA_SUMMARY_RELEASE_SLOT_FIELDS: frozenset[str] = frozenset(
    (
        "required",
        "native_bridge_abi_version",
        "device_family",
        "device_model",
        "device_codename",
        "signed_at_utc",
        "d2d_payment_transport",
        "d2d_payment_transports",
        D2D_PAYMENT_TRANSCRIPTS_FIELD,
        *KAGEMUSHA_SUMMARY_RELEASE_SHA256_FIELDS,
        *(path_field for path_field, _ in KAGEMUSHA_SUMMARY_RELEASE_ARTIFACTS),
    )
)
KAGEMUSHA_SUMMARY_RELEASE_REDACTED_SLOT_IDS: frozenset[str] = frozenset(
    {
        SECRET_PATH_REDACTION,
        CONTROL_PATH_REDACTION,
        "<unsafe-slot-name>",
    }
)


def _summary_release_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and SHA256_HEX_RE.fullmatch(value) is not None
        and value != "0" * 64
    )


def _summary_release_artifact_path(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    errors: list[str] = []
    return (
        _normalise_safe_relative_path(
            value,
            errors,
            "Kagemusha summary artifact path",
        )
        == value
        and not errors
    )


def _summary_release_d2d_transcript_path(value: Any) -> bool:
    return (
        _summary_release_artifact_path(value)
        and isinstance(value, str)
        and _safe_relative_path_is_child_of(value, "handoff")
    )


def _summary_release_artifact_path_under(value: Any, root: str) -> bool:
    return (
        _summary_release_artifact_path(value)
        and isinstance(value, str)
        and _safe_relative_path_is_child_of(value, root)
    )


def _summary_release_timestamp(value: Any) -> bool:
    if not isinstance(value, str) or SIGNED_AT_UTC_RE.fullmatch(value) is None:
        return False
    try:
        dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ")
    except ValueError:
        return False
    return True


def _summary_release_slot_id(value: Any) -> str | None:
    if not isinstance(value, str) or value in KAGEMUSHA_SUMMARY_RELEASE_REDACTED_SLOT_IDS:
        return None
    slot_ids, slot_errors = validate_slot_ids([value])
    if slot_errors or slot_ids != [value]:
        return None
    return value


def _summary_release_kagemusha(
    report: dict,
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> dict[str, Any] | None:
    """Return complete release Kagemusha details for safe summary rollups."""

    if report.get("status") != "ok" or _summary_release_slot_id(report.get("slot")) is None:
        return None
    kagemusha = _summary_kagemusha(report)
    if kagemusha.get("required") is not True:
        return None
    family = kagemusha.get("device_family")
    model = kagemusha.get("device_model")
    codename = kagemusha.get("device_codename")
    if (
        not isinstance(family, str)
        or family not in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
        or not isinstance(model, str)
        or not model
        or model != model.strip()
        or _contains_control_character(model)
        or SECRET_RE.search(model)
        or not isinstance(codename, str)
        or not codename
        or codename != codename.strip()
        or _contains_control_character(codename)
        or SECRET_RE.search(codename)
        or infer_kagemusha_device_family(model, codename) != family
    ):
        return None
    abi_version = kagemusha.get("native_bridge_abi_version")
    if (
        isinstance(abi_version, bool)
        or not isinstance(abi_version, int)
        or abi_version != REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION
    ):
        return None
    if not _summary_release_timestamp(kagemusha.get("signed_at_utc")):
        return None
    if any(
        not _summary_release_sha256(kagemusha.get(field))
        for field in KAGEMUSHA_SUMMARY_RELEASE_SHA256_FIELDS
    ):
        return None
    signer_public_key_sha256 = kagemusha.get(
        "signed_evidence_signer_public_key_sha256"
    )
    if (
        trusted_signer_public_key_sha256 is not None
        and signer_public_key_sha256 not in trusted_signer_public_key_sha256
    ):
        return None
    for path_field, _ in KAGEMUSHA_SUMMARY_RELEASE_ARTIFACTS:
        root = KAGEMUSHA_SUMMARY_RELEASE_ARTIFACT_ROOTS[path_field]
        if not _summary_release_artifact_path_under(kagemusha.get(path_field), root):
            return None
    return kagemusha


def _summary_release_device_family(
    report: dict,
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> str | None:
    """Return the device family only for a complete release Kagemusha report."""

    kagemusha = _summary_release_kagemusha(
        report,
        trusted_signer_public_key_sha256,
    )
    if kagemusha is None:
        return None
    family = kagemusha.get("device_family")
    return family if isinstance(family, str) else None


def _summary_release_d2d_payment_transport(
    report: dict,
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> str | None:
    """Return the D2D payment transport only for a complete release Kagemusha report."""

    kagemusha = _summary_release_kagemusha(
        report,
        trusted_signer_public_key_sha256,
    )
    if kagemusha is None:
        return None
    transport = kagemusha.get("d2d_payment_transport")
    if isinstance(transport, str) and transport in D2D_PAYMENT_TRANSPORTS:
        return transport
    return None


def _summary_release_d2d_transcript_binding(value: Any) -> tuple[str, str] | None:
    """Return a validated D2D transcript path/digest binding from a summary."""

    if not isinstance(value, dict) or set(value) != {"path", "sha256"}:
        return None
    path = value.get("path")
    digest = value.get("sha256")
    if not _summary_release_d2d_transcript_path(path) or not _summary_release_sha256(
        digest
    ):
        return None
    return path, digest


def _summary_release_d2d_transcript_bindings_are_exact(
    kagemusha: dict[str, Any],
    declared_transports: set[str],
    primary_transport: str,
) -> bool:
    """Return whether declared D2D transports exactly match transcript bindings."""

    transcripts = kagemusha.get(D2D_PAYMENT_TRANSCRIPTS_FIELD)
    if not isinstance(transcripts, dict) or set(transcripts) != declared_transports:
        return False
    primary_path = kagemusha.get("d2d_payment_transcript_path")
    primary_digest = kagemusha.get("d2d_payment_transcript_sha256")
    for transport in declared_transports:
        binding = _summary_release_d2d_transcript_binding(transcripts.get(transport))
        if binding is None:
            return False
        if transport == primary_transport and binding != (primary_path, primary_digest):
            return False
    return True


def _summary_release_d2d_payment_transports(
    report: dict,
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> list[str]:
    """Return D2D transports only when transcript declarations are release-bound."""

    kagemusha = _summary_release_kagemusha(
        report,
        trusted_signer_public_key_sha256,
    )
    if kagemusha is None:
        return []
    primary_transport = _summary_release_d2d_payment_transport(
        report,
        trusted_signer_public_key_sha256,
    )
    transports = kagemusha.get("d2d_payment_transports")
    if isinstance(transports, list) and all(
        isinstance(transport, str) and transport in D2D_PAYMENT_TRANSPORTS
        for transport in transports
    ):
        if transports != sorted(set(transports)):
            return []
        declared_transports = set(transports)
        if (
            primary_transport is not None
            and primary_transport in declared_transports
            and _summary_release_d2d_transcript_bindings_are_exact(
                kagemusha,
                declared_transports,
                primary_transport,
            )
        ):
            return sorted(declared_transports)
        return []
    transcripts = kagemusha.get(D2D_PAYMENT_TRANSCRIPTS_FIELD)
    if primary_transport is None:
        return []
    if transcripts is None:
        return [primary_transport]
    if _summary_release_d2d_transcript_bindings_are_exact(
        kagemusha,
        {primary_transport},
        primary_transport,
    ):
        return [primary_transport]
    return []


def _summary_reports_for_release_output(
    reports: list[dict],
    *,
    require_complete_signed_evidence: bool,
    trusted_signer_public_key_sha256: frozenset[str] | None,
) -> list[dict]:
    """Return scanner summary rows with incomplete release Kagemusha claims pruned."""

    if not require_complete_signed_evidence:
        return reports
    pruned_reports: list[dict] = []
    for report in reports:
        summary = dict(report)
        if _summary_release_kagemusha(report, trusted_signer_public_key_sha256) is None:
            kagemusha = summary.get("kagemusha")
            if isinstance(kagemusha, dict):
                pruned = dict(kagemusha)
                for field in KAGEMUSHA_SUMMARY_RELEASE_SLOT_FIELDS:
                    pruned.pop(field, None)
                summary["kagemusha"] = pruned
            else:
                summary["kagemusha"] = {}
        pruned_reports.append(summary)
    return pruned_reports


def infer_kagemusha_device_family(
    model: str | None,
    codename: str | None,
) -> str | None:
    """Infer a standard Kagemusha device family from Android identity fields."""

    model_text = model.lower() if isinstance(model, str) else ""
    codename_text = codename.lower() if isinstance(codename, str) else ""
    model_family = _match_kagemusha_device_model_family(model_text)
    codename_family = _match_kagemusha_device_codename_family(codename_text)
    if model_family is None or codename_family is None:
        return None
    if model_family != codename_family:
        return None
    return model_family


def _match_kagemusha_device_model_family(model_text: str) -> str | None:
    for family, exact_models, _codenames, model_prefixes in (
        KAGEMUSHA_DEVICE_FAMILY_MODEL_RULES
    ):
        if model_text in exact_models:
            return family
        if any(model_text.startswith(prefix) for prefix in model_prefixes):
            return family
    return None


def _match_kagemusha_device_codename_family(codename_text: str) -> str | None:
    for family, _exact_models, codenames, _model_prefixes in (
        KAGEMUSHA_DEVICE_FAMILY_MODEL_RULES
    ):
        if codename_text in codenames:
            return family
    return None


def _normalise_manifest_path(path_text: str, errors: list[str], line_no: int) -> str | None:
    return _normalise_safe_relative_path(
        path_text,
        errors,
        f"sha256sum.txt line {line_no}",
    )


def validate_slot_ids(slot_ids: Iterable[str] | None) -> tuple[list[str] | None, list[str]]:
    """Validate explicit slot ids before they are joined to the lab root."""

    if not slot_ids:
        return None, []
    errors: list[str] = []
    normalised: list[str] = []
    seen: dict[str, int] = {}
    for index, raw_slot_id in enumerate(slot_ids):
        slot_id = raw_slot_id
        if not slot_id:
            errors.append(f"slot id {index} must be a non-empty string")
            continue
        if any(character.isspace() for character in slot_id):
            errors.append(f"slot id {index} must not contain whitespace")
            continue
        if _contains_control_character(slot_id):
            errors.append(f"slot id {index} must not contain control characters")
            continue
        if SECRET_RE.search(slot_id):
            errors.append(f"slot id {index} must not contain secret-looking material")
            continue
        candidate = PurePosixPath(slot_id)
        if (
            slot_id.startswith("/")
            or "\\" in slot_id
            or candidate.is_absolute()
            or len(candidate.parts) != 1
            or candidate.name in {"", ".", ".."}
            or ".." in candidate.parts
        ):
            errors.append(
                f"slot id {_display_path(slot_id)!r} must be a single safe directory name"
            )
            continue
        normalised_slot_id = candidate.name
        if candidate.as_posix() != slot_id:
            errors.append(
                f"slot id {_display_path(slot_id)!r} must be a canonical single directory name"
            )
            continue
        prior_index = seen.get(normalised_slot_id)
        if prior_index is not None:
            errors.append(
                f"slot id {index} must not duplicate slot id {prior_index}"
            )
            continue
        seen[normalised_slot_id] = index
        normalised.append(normalised_slot_id)
    return normalised, errors


def validate_device_lab_root_path(root: Path) -> list[str]:
    """Validate the device-lab root before slot discovery."""

    _root_exists, errors = classify_device_lab_root_path(root)
    return errors


def _path_has_surrounding_whitespace_component(path: Path) -> bool:
    """Return true when any path component has ambiguous surrounding whitespace."""

    return any(part != part.strip() for part in path.parts if part)


def classify_device_lab_root_path(root: Path) -> tuple[bool, list[str]]:
    """Classify whether the device-lab root exists and is safe for discovery."""

    root_text = str(root)
    if SECRET_RE.search(root_text):
        return False, ["device-lab root path must not contain secret-looking material"]
    if _contains_control_character(root_text):
        return False, ["device-lab root path must not contain control characters"]
    if root_text != root_text.strip() or _path_has_surrounding_whitespace_component(root):
        return False, ["device-lab root path must not contain surrounding whitespace"]
    if "\\" in root_text:
        return False, ["device-lab root path must not contain backslashes"]
    if ".." in root.parts:
        return False, ["device-lab root path must be canonical"]
    try:
        root_mode = root.lstat().st_mode
    except FileNotFoundError:
        root_mode = None
    except OSError:
        return False, ["device-lab root metadata could not be read"]
    if root_mode is not None and stat.S_ISLNK(root_mode):
        return True, ["device-lab root must not be a symlink"]
    errors = validate_no_symlink_ancestors(
        root,
        "device-lab root ancestor directory",
    )
    if errors:
        return root_mode is not None, errors
    if root_mode is not None and not stat.S_ISDIR(root_mode):
        return True, ["device-lab root must be a directory"]
    return root_mode is not None, []


def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:
    """Reject direct helper calls that receive unsafe slot path spellings."""

    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        errors.extend(path_errors)
        return True
    return False


def _slot_path_boundary_errors(slot_path: Path) -> list[str]:
    """Reject direct helper calls that receive unsafe slot path spellings."""

    path_text = str(slot_path)
    if SECRET_RE.search(path_text):
        return ["slot path must not contain secret-looking material"]
    if _contains_control_character(path_text):
        return ["slot path must not contain control characters"]
    if path_text != path_text.strip() or _path_has_surrounding_whitespace_component(
        slot_path
    ):
        return ["slot path must not contain surrounding whitespace"]
    if "\\" in path_text:
        return ["slot path must not contain backslashes"]
    if ".." in slot_path.parts:
        return ["slot path must be canonical"]
    return []


def _cli_path_alias_errors(path: str, label: str) -> list[str]:
    """Reject CLI path aliases before scanner metadata reads."""

    candidate = Path(path)
    if path != path.strip() or _path_has_surrounding_whitespace_component(candidate):
        return [f"{label} must not contain surrounding whitespace"]
    if "\\" in path:
        return [f"{label} must not contain backslashes"]
    if ".." in candidate.parts:
        return [f"{label} must be a canonical path"]
    return []


def _append_error_once(errors: list[str], message: str) -> None:
    if message not in errors:
        errors.append(message)


def _slot_tree_entries(
    dir_path: Path, label: str, errors: list[str]
) -> list[Path] | None:
    entries: list[Path] = []
    pending = [dir_path]
    while pending:
        current = pending.pop()
        try:
            scanned = sorted(os.scandir(current), key=lambda entry: entry.name)
        except OSError:
            _append_error_once(errors, f"{label} could not be listed")
            return None
        for entry in scanned:
            entry_path = Path(entry.path)
            entries.append(entry_path)
            try:
                entry_mode = entry.stat(follow_symlinks=False).st_mode
            except OSError:
                continue
            if stat.S_ISDIR(entry_mode):
                pending.append(entry_path)
    return entries

def validate_no_symlink_ancestors(path: Path, label: str) -> list[str]:
    """Reject symlinked parent directories without leaking local paths."""

    if path.is_absolute():
        candidate = path
    else:
        try:
            candidate = Path.cwd() / path
        except OSError:
            return [f"{label} metadata could not be read"]
    errors: list[str] = []
    for ancestor in candidate.parents:
        if ancestor.is_absolute() and len(ancestor.parts) <= 2:
            continue
        try:
            ancestor_mode = ancestor.lstat().st_mode
        except FileNotFoundError:
            continue
        except OSError:
            errors.append(f"{label} metadata could not be read")
            break
        if stat.S_ISLNK(ancestor_mode):
            errors.append(f"{label} must not be a symlink")
            break
    return errors


def _validate_manifest_slot_path(slot_path: Path) -> list[str]:
    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        return path_errors
    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        slot_mode = None
    except OSError:
        return ["slot directory metadata could not be read"]
    if slot_mode is not None and stat.S_ISLNK(slot_mode):
        return ["slot directory must not be a symlink"]
    return validate_no_symlink_ancestors(slot_path, "slot ancestor directory")


def parse_sha256_manifest(slot_path: Path) -> tuple[dict[str, str], list[str]]:
    """Parse and validate the slot's sha256sum.txt manifest."""

    entries: dict[str, str] = {}
    root_errors = _validate_manifest_slot_path(slot_path)
    if root_errors:
        return entries, root_errors
    errors: list[str] = []
    manifest_path = slot_path / "sha256sum.txt"
    try:
        manifest_stat = manifest_path.lstat()
    except FileNotFoundError:
        return entries, ["missing sha256sum.txt"]
    except OSError:
        return entries, ["sha256sum.txt file metadata could not be read"]
    if stat.S_ISLNK(manifest_stat.st_mode):
        return entries, ["sha256sum.txt must not be a symlink"]
    if not stat.S_ISREG(manifest_stat.st_mode):
        return entries, ["sha256sum.txt must be a regular file"]
    try:
        if manifest_path.stat().st_nlink > 1:
            return entries, ["sha256sum.txt must not be hardlinked"]
    except OSError:
        return entries, ["sha256sum.txt hardlink metadata could not be read"]

    try:
        with manifest_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = manifest_path.lstat()
            expected_identity = (manifest_stat.st_dev, manifest_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if stat.S_ISLNK(path_stat.st_mode):
                return entries, ["sha256sum.txt must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return entries, ["sha256sum.txt must be a regular file"]
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return entries, ["sha256sum.txt changed while being read"]
            if open_stat.st_nlink > 1:
                return entries, ["sha256sum.txt must not be hardlinked"]
            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES:
                return entries, [
                    "sha256sum.txt must be no more than "
                    f"{MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES} bytes"
                ]
            chunks: list[bytes] = []
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES:
                    return entries, [
                        "sha256sum.txt must be no more than "
                        f"{MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = manifest_path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return entries, ["sha256sum.txt changed while being read"]
        lines = b"".join(chunks).decode("utf-8").splitlines()
    except (OSError, UnicodeDecodeError):
        return entries, ["sha256sum.txt could not be read"]
    for line_no, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line:
            continue
        if raw != line:
            errors.append(
                f"sha256sum.txt line {line_no}: must not contain surrounding whitespace"
            )
            continue
        if line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            errors.append(f"sha256sum.txt line {line_no}: expected '<sha256> <path>'")
            continue
        digest, path_text = parts
        if not SHA256_HEX_RE.fullmatch(digest) or digest == "0" * 64:
            errors.append(f"sha256sum.txt line {line_no}: non-canonical sha256 digest")
            continue
        relative = _normalise_manifest_path(path_text, errors, line_no)
        if relative is None:
            continue
        if relative in entries:
            errors.append(
                f"sha256sum.txt line {line_no}: duplicate entry for {_display_path(relative)}"
            )
            continue
        entries[relative] = digest

    if not entries and not errors:
        errors.append("sha256sum.txt is empty")
    return entries, errors


def _has_manifest_file_shape_error(errors: list[str]) -> bool:
    return any(
        error
        in {
            "missing sha256sum.txt",
            "sha256sum.txt must not be a symlink",
            "sha256sum.txt must be a regular file",
            "sha256sum.txt must not be hardlinked",
            "sha256sum.txt hardlink metadata could not be read",
        }
        for error in errors
    )


def _slot_artifact_lstat_mode(
    artifact_path: Path,
    metadata_error: str,
) -> tuple[int | None, list[str]]:
    try:
        return artifact_path.lstat().st_mode, []
    except FileNotFoundError:
        return None, []
    except OSError:
        return None, [metadata_error]


def _validate_manifest_artifact_for_digest(
    slot_path: Path,
    relative: str,
) -> tuple[Path | None, os.stat_result | None, list[str]]:
    """Validate one manifest artifact immediately before hashing it."""

    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        return None, None, path_errors
    if SECRET_RE.search(relative):
        return None, None, ["slot artifacts must not contain secret-looking material"]
    normalise_errors: list[str] = []
    safe_relative = _normalise_safe_relative_path(
        relative,
        normalise_errors,
        "sha256sum.txt artifact path",
        allow_sha_manifest=True,
    )
    if normalise_errors:
        return None, None, normalise_errors
    assert safe_relative is not None
    display = _display_path(safe_relative)
    artifact_path = slot_path / safe_relative
    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:
        return None, None, [
            "sha256sum.txt references artifact under symlink directory "
            f"{display}"
        ]
    try:
        artifact_stat = artifact_path.lstat()
    except FileNotFoundError:
        return None, None, [f"sha256sum.txt references missing file {display}"]
    except OSError:
        return None, None, [
            f"sha256sum.txt references artifact file metadata could not be read {display}"
        ]
    if stat.S_ISLNK(artifact_stat.st_mode):
        return None, None, [f"sha256sum.txt references symlink artifact {display}"]
    if not stat.S_ISREG(artifact_stat.st_mode):
        return None, None, [f"sha256sum.txt references non-regular artifact {display}"]
    if artifact_stat.st_nlink > 1:
        return None, None, [f"sha256sum.txt references hardlinked artifact {display}"]
    return artifact_path, artifact_stat, []


def _read_validated_manifest_artifact_bytes(
    artifact_path: Path,
    expected_stat: os.stat_result,
    relative: str,
    max_bytes: int = MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES,
) -> tuple[bytes | None, list[str]]:
    """Read a manifest artifact without trusting a stale path."""

    display = _display_path(relative)
    chunks: list[bytes] = []
    try:
        with artifact_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = artifact_path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"sha256sum.txt references symlink artifact {display}"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [
                    f"sha256sum.txt references non-regular artifact {display}"
                ]
            manifest_expected_identity = (
                expected_stat.st_dev,
                expected_stat.st_ino,
            )
            manifest_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if manifest_open_identity != manifest_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != manifest_expected_identity:
                return None, [
                    "sha256sum.txt references artifact changed while being read "
                    f"{display}"
                ]
            if open_stat.st_nlink > 1:
                return None, [
                    f"sha256sum.txt references hardlinked artifact {display}"
                ]
            if open_stat.st_size > max_bytes:
                return None, [
                    "sha256sum.txt references artifact "
                    f"{display} must be no more than "
                    f"{max_bytes} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [
                        "sha256sum.txt references artifact "
                        f"{display} must be no more than "
                        f"{max_bytes} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = artifact_path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != manifest_expected_identity:
                return None, [
                    "sha256sum.txt references artifact changed while being read "
                    f"{display}"
                ]
    except OSError:
        return None, [
            "sha256sum.txt references artifact that could not be read "
            f"{display}"
        ]
    return b"".join(chunks), []


def _manifest_artifact_sha256(
    slot_path: Path,
    relative: str,
) -> tuple[str | None, list[str]]:
    artifact_path, artifact_stat, errors = _validate_manifest_artifact_for_digest(
        slot_path,
        relative,
    )
    if errors:
        return None, errors
    assert artifact_path is not None and artifact_stat is not None
    payload, read_errors = _read_validated_manifest_artifact_bytes(
        artifact_path,
        artifact_stat,
        relative,
        _slot_artifact_max_bytes(relative),
    )
    if read_errors:
        return None, read_errors
    assert payload is not None
    return hashlib.sha256(payload).hexdigest(), []


def _validate_signed_evidence_artifact_for_digest(
    slot_path: Path,
    relative: str,
) -> tuple[Path | None, os.stat_result | None, list[str]]:
    """Validate one signed-evidence artifact immediately before hashing it."""

    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        return None, None, path_errors
    if SECRET_RE.search(relative):
        return None, None, [
            "signed evidence artifact digest path must not contain secret-looking material"
        ]
    normalise_errors: list[str] = []
    safe_relative = _normalise_safe_relative_path(
        relative,
        normalise_errors,
        "signed evidence artifact digest path",
    )
    if normalise_errors:
        return None, None, normalise_errors
    assert safe_relative is not None
    display = _display_path(safe_relative)
    artifact_path = slot_path / safe_relative
    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:
        return None, None, [
            "signed evidence artifact digest references artifact under "
            f"symlink directory {display}"
        ]
    try:
        artifact_stat = artifact_path.lstat()
    except FileNotFoundError:
        return None, None, [
            "signed evidence artifact required slot artifact is missing "
            f"{display}"
        ]
    except OSError:
        return None, None, [
            "signed evidence artifact digest references artifact file metadata "
            f"could not be read {display}"
        ]
    if stat.S_ISLNK(artifact_stat.st_mode):
        return None, None, [
            f"signed evidence artifact digest references symlink artifact {display}"
        ]
    if not stat.S_ISREG(artifact_stat.st_mode):
        return None, None, [
            f"signed evidence artifact digest references non-regular artifact {display}"
        ]
    if artifact_stat.st_nlink > 1:
        return None, None, [
            f"signed evidence artifact digest references hardlinked artifact {display}"
        ]
    return artifact_path, artifact_stat, []


def _read_validated_signed_evidence_artifact_bytes(
    artifact_path: Path,
    expected_stat: os.stat_result,
    relative: str,
    max_bytes: int = MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES,
) -> tuple[bytes | None, list[str]]:
    """Read a signed-evidence digest artifact without trusting a stale path."""

    display = _display_path(relative)
    chunks: list[bytes] = []
    try:
        with artifact_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = artifact_path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [
                    f"signed evidence artifact digest references symlink artifact {display}"
                ]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [
                    "signed evidence artifact digest references non-regular artifact "
                    f"{display}"
                ]
            signed_evidence_expected_identity = (
                expected_stat.st_dev,
                expected_stat.st_ino,
            )
            signed_evidence_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if signed_evidence_open_identity != signed_evidence_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != signed_evidence_expected_identity:
                return None, [
                    "signed evidence artifact digest references artifact changed "
                    f"while being read {display}"
                ]
            if open_stat.st_nlink > 1:
                return None, [
                    f"signed evidence artifact digest references hardlinked artifact {display}"
                ]
            if open_stat.st_size > max_bytes:
                return None, [
                    "signed evidence artifact digest references artifact "
                    f"{display} must be no more than "
                    f"{max_bytes} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [
                        "signed evidence artifact digest references artifact "
                        f"{display} must be no more than "
                        f"{max_bytes} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = artifact_path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != signed_evidence_expected_identity:
                return None, [
                    "signed evidence artifact digest references artifact changed "
                    f"while being read {display}"
                ]
    except OSError:
        return None, [
            "signed evidence artifact digest references artifact that could not be read "
            f"{display}"
        ]
    return b"".join(chunks), []


def _signed_evidence_artifact_sha256(
    slot_path: Path,
    relative: str,
) -> tuple[str | None, list[str]]:
    artifact_path, artifact_stat, errors = _validate_signed_evidence_artifact_for_digest(
        slot_path,
        relative,
    )
    if errors:
        return None, errors
    assert artifact_path is not None and artifact_stat is not None
    payload, read_errors = _read_validated_signed_evidence_artifact_bytes(
        artifact_path,
        artifact_stat,
        relative,
        _slot_artifact_max_bytes(relative),
    )
    if read_errors:
        return None, read_errors
    assert payload is not None
    return hashlib.sha256(payload).hexdigest(), []


def _validate_metadata_artifact_for_read(
    slot_path: Path,
    relative: str,
    label: str,
    missing_error: str,
) -> tuple[Path | None, os.stat_result | None, list[str]]:
    """Validate a slot-relative metadata artifact immediately before reading it."""

    path_errors = _slot_path_boundary_errors(slot_path)
    if path_errors:
        return None, None, path_errors
    if SECRET_RE.search(relative):
        return None, None, [f"{label} must not contain secret-looking material"]
    normalise_errors: list[str] = []
    safe_relative = _normalise_safe_relative_path(
        relative,
        normalise_errors,
        label,
    )
    if normalise_errors:
        return None, None, normalise_errors
    assert safe_relative is not None
    display = _display_path(safe_relative)
    artifact_path = slot_path / safe_relative
    if _slot_relative_symlink_ancestor(slot_path, safe_relative) is not None:
        return None, None, [
            f"{label} references artifact under symlink directory {display}"
        ]
    try:
        artifact_stat = artifact_path.lstat()
    except FileNotFoundError:
        return None, None, [missing_error]
    except OSError:
        return None, None, [
            f"{label} references artifact file metadata could not be read {display}"
        ]
    if stat.S_ISLNK(artifact_stat.st_mode):
        return None, None, [f"{label} references symlink artifact {display}"]
    if not stat.S_ISREG(artifact_stat.st_mode):
        return None, None, [f"{label} references non-regular artifact {display}"]
    if artifact_stat.st_nlink > 1:
        return None, None, [f"{label} references hardlinked artifact {display}"]
    return artifact_path, artifact_stat, []


def _read_validated_metadata_artifact_bytes(
    artifact_path: Path,
    expected_stat: os.stat_result,
    label: str,
    relative: str,
    unreadable_error: str,
    max_bytes: int = MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES,
) -> tuple[bytes | None, list[str]]:
    """Read an already validated metadata artifact without trusting a stale path."""

    digest_chunks: list[bytes] = []
    try:
        with artifact_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = artifact_path.lstat()
            display = _display_path(relative)
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [f"{label} references symlink artifact {display}"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(open_stat.st_mode):
                return None, [f"{label} references non-regular artifact {display}"]
            expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
            open_identity = (open_stat.st_dev, open_stat.st_ino)
            if open_identity != expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != expected_identity:
                return None, [
                    f"{label} references artifact changed while being read {display}"
                ]
            if open_stat.st_nlink > 1:
                return None, [f"{label} references hardlinked artifact {display}"]
            if open_stat.st_size > max_bytes:
                return None, [
                    f"{label} references artifact {display} must be no more than "
                    f"{max_bytes} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > max_bytes:
                    return None, [
                        f"{label} references artifact {display} must be no more than "
                        f"{max_bytes} bytes"
                    ]
                digest_chunks.append(chunk)
            final_path_stat = artifact_path.lstat()
            if (final_path_stat.st_dev, final_path_stat.st_ino) != expected_identity:
                return None, [
                    f"{label} references artifact changed while being read {display}"
                ]
    except OSError:
        return None, [unreadable_error]
    return b"".join(digest_chunks), []


def _metadata_artifact_bytes_and_sha256(
    slot_path: Path,
    relative: str,
    label: str,
    missing_error: str,
    max_bytes: int = MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES,
) -> tuple[bytes | None, str | None, list[str]]:
    """Validate a slot.json-referenced artifact immediately before reading it."""

    artifact_path, artifact_stat, errors = _validate_metadata_artifact_for_read(
        slot_path,
        relative,
        label,
        missing_error,
    )
    if errors:
        return None, None, errors
    assert artifact_path is not None and artifact_stat is not None
    artifact_bytes, read_errors = _read_validated_metadata_artifact_bytes(
        artifact_path,
        artifact_stat,
        label,
        relative,
        f"{label} could not be read",
        max_bytes,
    )
    if read_errors:
        return None, None, read_errors
    assert artifact_bytes is not None
    return artifact_bytes, hashlib.sha256(artifact_bytes).hexdigest(), []


def _metadata_artifact_text(
    slot_path: Path,
    relative: str,
    label: str,
    missing_error: str,
    unreadable_error: str,
    *,
    decode_errors: str = "strict",
) -> tuple[str | None, list[str]]:
    """Validate a slot-relative text artifact immediately before reading it."""

    artifact_path, artifact_stat, errors = _validate_metadata_artifact_for_read(
        slot_path,
        relative,
        label,
        missing_error,
    )
    if errors:
        return None, errors
    assert artifact_path is not None and artifact_stat is not None
    artifact_bytes, read_errors = _read_validated_metadata_artifact_bytes(
        artifact_path,
        artifact_stat,
        label,
        relative,
        unreadable_error,
    )
    if read_errors:
        return None, read_errors
    assert artifact_bytes is not None
    try:
        return artifact_bytes.decode("utf-8", errors=decode_errors), []
    except UnicodeDecodeError:
        return None, [unreadable_error]


def _should_read_optional_text_artifact(
    slot_path: Path,
    relative: str,
    label: str,
    errors: list[str],
) -> bool:
    mode, mode_errors = _slot_artifact_lstat_mode(
        slot_path / relative,
        f"{label} file metadata could not be read",
    )
    if mode_errors:
        errors.extend(mode_errors)
        return False
    if mode is None:
        return False
    return stat.S_ISLNK(mode) or stat.S_ISREG(mode)


def verify_sha256_manifest(slot_path: Path) -> list[str]:
    """Check that sha256sum.txt exactly covers the slot artefacts."""

    root_errors = _validate_manifest_slot_path(slot_path)
    if root_errors:
        return root_errors
    entries, errors = parse_sha256_manifest(slot_path)
    if _has_manifest_file_shape_error(errors):
        return errors
    actual_files = _slot_files(slot_path, errors)

    for relative, expected_digest in sorted(entries.items()):
        actual_digest, digest_errors = _manifest_artifact_sha256(slot_path, relative)
        if digest_errors:
            errors.extend(digest_errors)
            continue
        assert actual_digest is not None
        if actual_digest != expected_digest:
            errors.append(f"sha256sum.txt digest mismatch for {_display_path(relative)}")

    for relative in sorted(actual_files - set(entries)):
        errors.append(f"sha256sum.txt missing entry for {_display_path(relative)}")

    return errors


class DuplicateJsonKeyError(ValueError):
    """Raised when a JSON object repeats a key."""

    def __init__(self, key: str) -> None:
        self.key = key
        super().__init__(key)


class NonFiniteJsonConstantError(ValueError):
    """Raised when JSON text uses a non-standard NaN/Infinity constant."""

    def __init__(self, constant: str) -> None:
        self.constant = constant
        super().__init__(constant)


def _reject_nonfinite_json_constant(constant: str) -> None:
    raise NonFiniteJsonConstantError(constant)


def _reject_duplicate_json_object_pairs(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    item: dict[str, Any] = {}
    for key, value in pairs:
        if key in item:
            raise DuplicateJsonKeyError(key)
        item[key] = value
    return item


def _loads_json_without_duplicate_keys(text: str) -> Any:
    return json.loads(
        text,
        object_pairs_hook=_reject_duplicate_json_object_pairs,
        parse_constant=_reject_nonfinite_json_constant,
    )


def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:
    path_text = str(path)
    if SECRET_RE.search(path_text):
        errors.append(f"{label} path must not contain secret-looking material")
        return None
    if _contains_control_character(path_text):
        errors.append(f"{label} path must not contain control characters")
        return None
    if path_text != path_text.strip() or _path_has_surrounding_whitespace_component(
        path
    ):
        errors.append(f"{label} path must not contain surrounding whitespace")
        return None
    if "\\" in path_text:
        errors.append(f"{label} path must not contain backslashes")
        return None
    if ".." in path.parts:
        errors.append(f"{label} path must be canonical")
        return None
    json_ancestor_errors = validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if json_ancestor_errors:
        errors.extend(json_ancestor_errors)
        return None
    try:
        expected_stat = path.lstat()
    except FileNotFoundError:
        errors.append(f"missing {label}")
        return None
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return None
    if stat.S_ISLNK(expected_stat.st_mode):
        errors.append(f"{label} must not be a symlink")
        return None
    if not stat.S_ISREG(expected_stat.st_mode):
        errors.append(f"{label} must be a regular file")
        return None
    try:
        link_count = path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return None
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
        return None
    try:
        chunks: list[bytes] = []
        json_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            json_path_stat = path.lstat()
            if stat.S_ISLNK(json_path_stat.st_mode):
                errors.append(f"{label} must not be a symlink")
                return None
            if not stat.S_ISREG(json_path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                errors.append(f"{label} must be a regular file")
                return None
            json_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if json_open_identity != json_expected_identity or (
                json_path_stat.st_dev,
                json_path_stat.st_ino,
            ) != json_expected_identity:
                errors.append(f"{label} changed while being read")
                return None
            if open_stat.st_nlink > 1:
                errors.append(f"{label} must not be hardlinked")
                return None
            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                errors.append(
                    f"{label} must be no more than "
                    f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                )
                return None
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                    errors.append(
                        f"{label} must be no more than "
                        f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                    )
                    return None
                chunks.append(chunk)
            json_final_path_stat = path.lstat()
            if (json_final_path_stat.st_dev, json_final_path_stat.st_ino) != (
                json_expected_identity
            ):
                errors.append(f"{label} changed while being read")
                return None
        data = _loads_json_without_duplicate_keys(b"".join(chunks).decode("utf-8"))
    except (OSError, UnicodeDecodeError):
        errors.append(f"{label} could not be read")
        return None
    except json.JSONDecodeError as exc:
        errors.append(f"{label} is not valid JSON: {exc}")
        return None
    except DuplicateJsonKeyError as exc:
        errors.append(
            f"{label} contains duplicate JSON object key {_display_path(exc.key)}"
        )
        return None
    except NonFiniteJsonConstantError as exc:
        errors.append(f"{label} contains non-finite constant {exc.constant}")
        return None
    if not isinstance(data, dict):
        errors.append(f"{label} must be a JSON object")
        return None
    return data


def validate_slot_metadata_fields(metadata: dict[str, Any], errors: list[str]) -> None:
    """Reject production slot metadata outside the signed evidence contract."""

    for field in sorted(set(metadata) - SLOT_METADATA_FIELDS):
        errors.append(f"slot.json contains unexpected field {_display_path(field)}")


def _require_non_empty_string(
    data: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"slot.json {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"slot.json {key} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"slot.json {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(f"slot.json {key} must not contain secret-looking material")
        return None
    return value


def _require_true(data: dict[str, Any], key: str, errors: list[str]) -> None:
    if data.get(key) is not True:
        errors.append(f"slot.json {key} must be true")


def _require_int(
    data: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> int | None:
    value = data.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        errors.append(f"{label} {key} must be an integer")
        return None
    return value


def _require_evidence_true(data: dict[str, Any], key: str, errors: list[str]) -> None:
    if data.get(key) is not True:
        errors.append(f"signed evidence artifact {key} must be true")


def _require_evidence_int(
    data: dict[str, Any],
    key: str,
    errors: list[str],
) -> int | None:
    value = data.get(key)
    if not isinstance(value, int) or isinstance(value, bool):
        errors.append(f"signed evidence artifact {key} must be an integer")
        return None
    return value


def _require_status(data: dict[str, Any], key: str, accepted: set[str], errors: list[str]) -> None:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"slot.json {key} must be a non-empty string")
        return
    if value != value.strip():
        errors.append(f"slot.json {key} must not contain surrounding whitespace")
        return
    if _contains_control_character(value):
        errors.append(f"slot.json {key} must not contain control characters")
        return
    if value != value.lower():
        errors.append(f"slot.json {key} must be lowercase")
        return
    if value not in accepted:
        errors.append(f"slot.json {key} must be one of {sorted(accepted)}")


def _require_evidence_string(
    data: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"signed evidence artifact {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(
            f"signed evidence artifact {key} must not contain surrounding whitespace"
        )
        return None
    if _contains_control_character(value):
        errors.append(f"signed evidence artifact {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"signed evidence artifact {key} must not contain secret-looking material"
        )
        return None
    return value


def _require_evidence_raw_string(
    data: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"signed evidence artifact {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(
            f"signed evidence artifact {key} must not contain surrounding whitespace"
        )
        return None
    if _contains_control_character(value):
        errors.append(f"signed evidence artifact {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"signed evidence artifact {key} must not contain secret-looking material"
        )
        return None
    return value


def _validate_raw_test_command_markers(
    commands: list[Any],
    *,
    label: str,
    errors: list[str],
) -> None:
    if not all(isinstance(command, str) for command in commands):
        return
    rendered = "\n".join(commands)
    for marker in RAW_TEST_COMMAND_REQUIRED_MARKERS:
        if marker not in rendered:
            errors.append(f"{label} must include {marker}")
    if tuple(commands) != KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS:
        errors.append(
            f"{label} must exactly match the Kagemusha Android production raw test command"
        )


def _attestation_result_string(
    result: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = result.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"attestation/result.json {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"attestation/result.json {key} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"attestation/result.json {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"attestation/result.json {key} must not contain secret-looking material"
        )
        return None
    return value


def _attestation_result_matches_slot_metadata(
    result: dict[str, Any],
    metadata: dict[str, Any],
    key: str,
    errors: list[str],
) -> None:
    expected = metadata.get(key)
    actual = _attestation_result_string(result, key, errors)
    if key.endswith("_sha256") and actual is not None:
        if not SHA256_HEX_RE.fullmatch(actual):
            errors.append(f"attestation/result.json {key} must be lowercase sha256 hex")
        elif actual == "0" * 64:
            errors.append(
                f"attestation/result.json {key} must be non-zero lowercase sha256 hex"
            )
    if isinstance(expected, str) and actual is not None and actual != expected:
        errors.append(f"attestation/result.json {key} must match slot.json {key}")


def validate_attestation_result(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate production StrongBox/KeyMint attestation summary bindings."""

    if _reject_secret_slot_path(slot_path, errors):
        return None
    result = _load_json(slot_path / "attestation" / "result.json", "attestation/result.json", errors)
    if result is None:
        return None

    for field in sorted(set(result) - ATTESTATION_RESULT_FIELDS):
        errors.append(
            f"attestation/result.json contains unexpected field {_display_path(field)}"
        )

    status = _attestation_result_string(result, "status", errors)
    if status is not None and status != "ok":
        errors.append("attestation/result.json status must be ok")

    slot_bindings: list[str] = []
    for slot_key in ("slot_id", "slot"):
        slot_value = result.get(slot_key)
        if slot_value is None:
            continue
        if not isinstance(slot_value, str) or not slot_value:
            errors.append(f"attestation/result.json {slot_key} must be a non-empty string")
            continue
        if slot_value != slot_value.strip():
            errors.append(
                f"attestation/result.json {slot_key} must not contain surrounding whitespace"
            )
            continue
        if _contains_control_character(slot_value):
            errors.append(
                f"attestation/result.json {slot_key} must not contain control characters"
            )
            continue
        if SECRET_RE.search(slot_value):
            errors.append(
                f"attestation/result.json {slot_key} must not contain secret-looking material"
            )
            continue
        slot_binding = slot_value
        slot_bindings.append(slot_binding)
        if slot_binding != slot_path.name:
            errors.append(
                f"attestation/result.json {slot_key} must match the slot directory name"
            )
    if not slot_bindings:
        errors.append("attestation/result.json slot_id must be a non-empty string")
    elif len(set(slot_bindings)) != 1:
        errors.append("attestation/result.json slot and slot_id must match")

    if result.get("strongbox_attestation") is not True:
        errors.append("attestation/result.json strongbox_attestation must be true")
    if result.get("physical_device_attestation") is not True:
        errors.append("attestation/result.json physical_device_attestation must be true")

    strongbox_seen = False
    for level_key in (
        "keymint_security_level",
        "attestation_security_level",
        "keymaster_security_level",
    ):
        level = _attestation_result_string(result, level_key, errors)
        if level is None:
            continue
        if level in STRONGBOX_LEVELS:
            strongbox_seen = True
        else:
            errors.append(f"attestation/result.json {level_key} must be STRONGBOX")
        expected_level = metadata.get(level_key)
        if (
            level_key == "keymint_security_level"
            and isinstance(expected_level, str)
            and level != expected_level
        ):
            errors.append(
                "attestation/result.json keymint_security_level must match "
                "slot.json keymint_security_level"
            )
    if not strongbox_seen:
        errors.append("attestation/result.json must report STRONGBOX security level")

    for key in ATTESTATION_RESULT_SLOT_BINDING_FIELDS:
        _attestation_result_matches_slot_metadata(result, metadata, key, errors)
    return result


def _attestation_report_string(
    report: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = report.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"attestation/report.json {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"attestation/report.json {key} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"attestation/report.json {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"attestation/report.json {key} must not contain secret-looking material"
        )
        return None
    return value


def _attestation_report_matches_slot_metadata(
    report: dict[str, Any],
    metadata: dict[str, Any],
    key: str,
    errors: list[str],
) -> None:
    expected = metadata.get(key)
    actual = _attestation_report_string(report, key, errors)
    if key.endswith("_sha256") and actual is not None:
        if not SHA256_HEX_RE.fullmatch(actual):
            errors.append(f"attestation/report.json {key} must be lowercase sha256 hex")
        elif actual == "0" * 64:
            errors.append(
                f"attestation/report.json {key} must be non-zero lowercase sha256 hex"
            )
    if isinstance(expected, str) and actual is not None and actual != expected:
        errors.append(f"attestation/report.json {key} must match slot.json {key}")


def _attestation_report_verification_string(
    verification: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = verification.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"attestation/report.json verification.{key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(
            f"attestation/report.json verification.{key} must not contain surrounding whitespace"
        )
        return None
    if _contains_control_character(value):
        errors.append(
            f"attestation/report.json verification.{key} must not contain control characters"
        )
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"attestation/report.json verification.{key} must not contain secret-looking material"
        )
        return None
    return value


def validate_attestation_report(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> dict[str, Any] | None:
    """Validate the production StrongBox/KeyMint verifier report."""

    if _reject_secret_slot_path(slot_path, errors):
        return None
    report = _load_json(
        slot_path / "attestation" / "report.json",
        "attestation/report.json",
        errors,
    )
    if report is None:
        return None

    for field in sorted(set(report) - ATTESTATION_REPORT_FIELDS):
        errors.append(
            f"attestation/report.json contains unexpected field {_display_path(field)}"
        )

    if report.get("schema") != ATTESTATION_REPORT_SCHEMA:
        errors.append(f"attestation/report.json schema must be {ATTESTATION_REPORT_SCHEMA}")

    for key in ATTESTATION_REPORT_SLOT_BINDING_FIELDS:
        _attestation_report_matches_slot_metadata(report, metadata, key, errors)

    _attestation_report_string(report, "verifier", errors)
    verification = report.get("verification")
    if not isinstance(verification, dict):
        errors.append("attestation/report.json verification must be an object")
        return None
    for field in sorted(set(verification) - ATTESTATION_REPORT_VERIFICATION_FIELDS):
        errors.append(
            "attestation/report.json verification contains unexpected field "
            f"{_display_path(field)}"
        )
    status = _attestation_report_verification_string(verification, "status", errors)
    if status is not None and status != "ok":
        errors.append("attestation/report.json verification.status must be ok")
    if verification.get("strongbox_attestation") is not True:
        errors.append("attestation/report.json verification.strongbox_attestation must be true")
    if verification.get("physical_device_attestation") is not True:
        errors.append(
            "attestation/report.json verification.physical_device_attestation must be true"
        )

    for level_key in (
        "keymint_security_level",
        "attestation_security_level",
        "keymaster_security_level",
    ):
        value = _attestation_report_verification_string(verification, level_key, errors)
        if value is not None and value not in STRONGBOX_LEVELS:
            errors.append(
                f"attestation/report.json verification.{level_key} must be STRONGBOX"
            )
    return report


def validate_attestation_report_result_level_binding(
    attestation_result: dict[str, Any] | None,
    attestation_report: dict[str, Any] | None,
    errors: list[str],
) -> None:
    """Require verifier report status and levels to match the raw attestation result."""

    if attestation_result is None or attestation_report is None:
        return
    verification = attestation_report.get("verification")
    if not isinstance(verification, dict):
        return
    result_status = attestation_result.get("status")
    report_status = verification.get("status")
    if (
        isinstance(result_status, str)
        and isinstance(report_status, str)
        and result_status != report_status
    ):
        errors.append(
            "attestation/report.json verification.status must match "
            "attestation/result.json status"
        )
    for level_key in (
        "keymint_security_level",
        "attestation_security_level",
        "keymaster_security_level",
    ):
        result_level = attestation_result.get(level_key)
        report_level = verification.get(level_key)
        if (
            isinstance(result_level, str)
            and isinstance(report_level, str)
            and result_level != report_level
        ):
            errors.append(
                f"attestation/report.json verification.{level_key} must match "
                f"attestation/result.json {level_key}"
            )


def _attestation_harness_result_string(
    result: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = result.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"attestation/harness-result.json {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(
            f"attestation/harness-result.json {key} must not have surrounding whitespace"
        )
        return None
    if _contains_control_character(value):
        errors.append(f"attestation/harness-result.json {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"attestation/harness-result.json {key} must not contain secret-looking material"
        )
        return None
    return value


def _certificate_chain_pem_count(payload: bytes) -> int:
    return payload.count(b"-----BEGIN CERTIFICATE-----")


def validate_attestation_harness_result(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
    *,
    attestation_certificate_chain_bytes: bytes | None = None,
) -> None:
    """Validate the original StrongBox attestation harness result preserved in the slot."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    result = _load_json(
        slot_path / "attestation" / "harness-result.json",
        "attestation/harness-result.json",
        errors,
    )
    if result is None:
        return

    for field in sorted(set(result) - ATTESTATION_HARNESS_RESULT_FIELDS):
        errors.append(
            "attestation/harness-result.json contains unexpected field "
            f"{_display_path(field)}"
        )

    _attestation_harness_result_string(result, "alias", errors)
    for key in ("attestation_security_level", "keymaster_security_level"):
        level = _attestation_harness_result_string(result, key, errors)
        if level is not None and level not in STRONGBOX_LEVELS:
            errors.append(f"attestation/harness-result.json {key} must be STRONGBOX")

    if result.get("strongbox_attestation") is not True:
        errors.append("attestation/harness-result.json strongbox_attestation must be true")

    challenge_hex = _attestation_harness_result_string(result, "challenge_hex", errors)
    challenge: bytes | None = None
    if challenge_hex is not None:
        if (
            challenge_hex != challenge_hex.lower()
            or any(ch.isspace() for ch in challenge_hex)
            or not all(ch in "0123456789abcdef" for ch in challenge_hex)
        ):
            errors.append(
                "attestation/harness-result.json challenge_hex must be lowercase hexadecimal without whitespace"
            )
        elif len(challenge_hex) % 2 != 0:
            errors.append("attestation/harness-result.json challenge_hex must be even-length hex")
        else:
            try:
                challenge = bytes.fromhex(challenge_hex)
            except ValueError:
                errors.append("attestation/harness-result.json challenge_hex must be hex")
    expected_challenge_digest = metadata.get("attestation_challenge_sha256")
    if (
        challenge is not None
        and isinstance(expected_challenge_digest, str)
        and SHA256_HEX_RE.fullmatch(expected_challenge_digest)
        and hashlib.sha256(challenge).hexdigest() != expected_challenge_digest
    ):
        errors.append(
            "attestation/harness-result.json challenge_hex digest must match slot.json attestation_challenge_sha256"
        )

    chain_length = result.get("chain_length")
    if not isinstance(chain_length, int) or isinstance(chain_length, bool):
        errors.append("attestation/harness-result.json chain_length must be an integer")
    elif chain_length < 2:
        errors.append("attestation/harness-result.json chain_length must be at least 2")
    elif attestation_certificate_chain_bytes is not None:
        certificate_count = _certificate_chain_pem_count(attestation_certificate_chain_bytes)
        if certificate_count and chain_length != certificate_count:
            errors.append(
                "attestation/harness-result.json chain_length must match "
                "attestation certificate-chain certificate count"
            )


def _d2d_transcript_string(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"d2d payment transcript {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"d2d payment transcript {key} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"d2d payment transcript {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(f"d2d payment transcript {key} must not contain secret-looking material")
        return None
    return value


def _d2d_transcript_sha256(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"d2d payment transcript {key} must be lowercase sha256 hex")
        return None
    if value == "0" * 64:
        errors.append(
            f"d2d payment transcript {key} must be non-zero lowercase sha256 hex"
        )
        return None
    return value


def _d2d_transcript_true(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> None:
    if transcript.get(key) is not True:
        errors.append(f"d2d payment transcript {key} must be true")


def _wallet_transcript_string(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"wallet integrity transcript {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(
            f"wallet integrity transcript {key} must not contain surrounding whitespace"
        )
        return None
    if _contains_control_character(value):
        errors.append(
            f"wallet integrity transcript {key} must not contain control characters"
        )
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"wallet integrity transcript {key} must not contain secret-looking material"
        )
        return None
    return value


def _wallet_transcript_sha256(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"wallet integrity transcript {key} must be lowercase sha256 hex")
        return None
    if value == "0" * 64:
        errors.append(
            f"wallet integrity transcript {key} must be non-zero lowercase sha256 hex"
        )
        return None
    return value


def _wallet_transcript_true(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> None:
    if transcript.get(key) is not True:
        if key == "stale_snapshot_rejected":
            errors.append(
                "wallet integrity transcript stale_snapshot_rejected must be true"
            )
            return
        errors.append(f"wallet integrity transcript {key} must be true")


def _require_distinct_d2d_digests(
    transcript: dict[str, Any],
    before_key: str,
    after_key: str,
    errors: list[str],
) -> None:
    before = _d2d_transcript_sha256(transcript, before_key, errors)
    after = _d2d_transcript_sha256(transcript, after_key, errors)
    if before is not None and after is not None and before == after:
        errors.append(f"d2d payment transcript {before_key} must differ from {after_key}")


def _require_distinct_wallet_digests(
    transcript: dict[str, Any],
    before_key: str,
    after_key: str,
    errors: list[str],
) -> None:
    before = _wallet_transcript_sha256(transcript, before_key, errors)
    after = _wallet_transcript_sha256(transcript, after_key, errors)
    if before is not None and after is not None and before == after:
        if (
            before_key == "key_id_before_sha256"
            and after_key == "key_id_after_sha256"
        ):
            errors.append(
                "wallet integrity transcript key_id_before_sha256 must differ from key_id_after_sha256"
            )
            return
        errors.append(
            f"wallet integrity transcript {before_key} must differ from {after_key}"
        )


def validate_d2d_payment_transcript(
    slot_path: Path,
    transcript_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> str | None:
    """Validate the offline-offline D2D payment handoff transcript."""

    if _reject_secret_slot_path(slot_path, errors):
        return None
    transcript = _load_json(transcript_path, "d2d payment transcript", errors)
    if transcript is None:
        return None

    for field in sorted(set(transcript) - D2D_PAYMENT_TRANSCRIPT_FIELDS):
        errors.append(f"d2d payment transcript contains unexpected field {_display_path(field)}")

    if transcript.get("schema") != D2D_PAYMENT_TRANSCRIPT_SCHEMA:
        errors.append(f"d2d payment transcript schema must be {D2D_PAYMENT_TRANSCRIPT_SCHEMA}")

    for key in D2D_PAYMENT_TRANSCRIPT_SLOT_STRING_BINDINGS:
        actual = _d2d_transcript_string(transcript, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and actual is not None and actual != expected:
            errors.append(f"d2d payment transcript {key} must match slot.json {key}")

    for key in D2D_PAYMENT_TRANSCRIPT_SLOT_SHA256_BINDINGS:
        actual = _d2d_transcript_sha256(transcript, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and actual is not None and actual != expected:
            errors.append(f"d2d payment transcript {key} must match slot.json {key}")

    for key in D2D_PAYMENT_TRANSCRIPT_TRUE_FIELDS:
        _d2d_transcript_true(transcript, key, errors)

    transport = _d2d_transcript_string(transcript, "transport", errors)
    if transport is not None and transport not in D2D_PAYMENT_TRANSPORTS:
        errors.append(
            "d2d payment transcript transport must be one of "
            f"{sorted(D2D_PAYMENT_TRANSPORTS)}"
        )
        transport = None

    payload_schema = _d2d_transcript_string(transcript, "payload_schema", errors)
    if payload_schema is not None and payload_schema != D2D_PAYMENT_PAYLOAD_SCHEMA:
        errors.append(f"d2d payment transcript payload_schema must be {D2D_PAYMENT_PAYLOAD_SCHEMA}")

    payload_bytes = _require_int(
        transcript,
        "payload_bytes",
        "d2d payment transcript",
        errors,
    )
    if payload_bytes is not None:
        if payload_bytes <= 0:
            errors.append("d2d payment transcript payload_bytes must be positive")
        elif payload_bytes > MAX_D2D_PAYMENT_PAYLOAD_BYTES:
            errors.append(
                "d2d payment transcript payload_bytes must be no more than "
                f"{MAX_D2D_PAYMENT_PAYLOAD_BYTES}"
            )

    payload_sha256 = _d2d_transcript_sha256(transcript, "payload_sha256", errors)
    _d2d_transcript_sha256(transcript, "transport_session_id_sha256", errors)
    _d2d_transcript_sha256(transcript, "receiver_ack_sha256", errors)
    _d2d_transcript_sha256(transcript, "one_use_key_id_sha256", errors)
    _require_distinct_d2d_digests(
        transcript,
        "payer_wallet_state_before_sha256",
        "payer_wallet_state_after_sha256",
        errors,
    )
    _require_distinct_d2d_digests(
        transcript,
        "payee_wallet_state_before_sha256",
        "payee_wallet_state_after_sha256",
        errors,
    )
    received_payload_sha256 = _d2d_transcript_sha256(
        transcript,
        "received_payload_sha256",
        errors,
    )
    if (
        payload_sha256 is not None
        and received_payload_sha256 is not None
        and payload_sha256 != received_payload_sha256
    ):
        errors.append(
            "d2d payment transcript received_payload_sha256 must match payload_sha256"
        )

    queue_before_sha256 = _d2d_transcript_sha256(
        transcript,
        "queue_before_sha256",
        errors,
    )
    queue_after_sha256 = _d2d_transcript_sha256(
        transcript,
        "queue_after_sha256",
        errors,
    )
    _, actual_queue_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
        slot_path,
        "queue/pending_queue.json",
        "d2d payment transcript queue_after_sha256",
        "d2d payment transcript queue_after_sha256 requires queue/pending_queue.json",
    )
    if digest_errors:
        errors.extend(digest_errors)
    elif (
        actual_queue_digest is not None
        and queue_after_sha256 is not None
        and queue_after_sha256 != actual_queue_digest
    ):
        errors.append(
            "d2d payment transcript queue_after_sha256 must match queue/pending_queue.json"
        )
    if (
        queue_before_sha256 is not None
        and queue_after_sha256 is not None
        and queue_before_sha256 == queue_after_sha256
    ):
        errors.append(
            "d2d payment transcript queue_before_sha256 must differ from queue_after_sha256"
        )
    return transport


def validate_d2d_payment_transcript_binding(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> tuple[str | None, str | None, str | None]:
    """Validate the slot.json path/hash binding for the D2D payment transcript."""

    if _reject_secret_slot_path(slot_path, errors):
        return None, None, None
    digest = _require_lowercase_sha256_hex(
        metadata,
        "d2d_payment_transcript_sha256",
        "slot.json",
        errors,
    )
    relative = _require_non_empty_string(metadata, "d2d_payment_transcript_path", errors)
    if relative is not None:
        relative = _normalise_safe_relative_path(
            relative,
            errors,
            "slot.json d2d_payment_transcript_path",
        )
    if relative is None:
        return None, digest, None
    if not _safe_relative_path_is_child_of(relative, "handoff"):
        errors.append("slot.json d2d_payment_transcript_path must stay under handoff/")
        return relative, digest, None

    _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
        slot_path,
        relative,
        "slot.json d2d_payment_transcript_path",
        "slot.json d2d_payment_transcript_path must point to an existing file",
    )
    if digest_errors:
        errors.extend(digest_errors)
        return relative, None, None

    matched_digest: str | None = None
    if digest is not None and actual_digest is not None:
        if actual_digest != digest:
            errors.append(
                "slot.json d2d_payment_transcript_sha256 does not match d2d_payment_transcript_path"
            )
        else:
            matched_digest = digest
    transcript_path = slot_path / relative
    transport = validate_d2d_payment_transcript(slot_path, transcript_path, metadata, errors)
    return relative, matched_digest, transport


def _require_d2d_transcript_entry_string(
    entry: dict[str, Any],
    key: str,
    label: str,
    errors: list[str],
) -> str | None:
    value = entry.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"{label} {key} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"{label} {key} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"{label} {key} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(f"{label} {key} must not contain secret-looking material")
        return None
    return value


def _validate_d2d_payment_transcript_entry(
    slot_path: Path,
    metadata: dict[str, Any],
    transport_key: str,
    entry: Any,
    errors: list[str],
) -> tuple[str | None, dict[str, str] | None]:
    label = f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD}[{transport_key}]"
    if not isinstance(entry, dict):
        errors.append(f"{label} must be an object")
        return None, None
    for field in sorted(set(entry) - D2D_PAYMENT_TRANSCRIPT_ENTRY_FIELDS):
        errors.append(f"{label} contains unexpected field {_display_path(field)}")
    for field in sorted(D2D_PAYMENT_TRANSCRIPT_ENTRY_FIELDS - set(entry)):
        errors.append(f"{label} is missing {field}")
    relative = _require_d2d_transcript_entry_string(entry, "path", label, errors)
    digest = _require_d2d_transcript_entry_string(entry, "sha256", label, errors)
    if digest is not None:
        if SHA256_HEX_RE.fullmatch(digest) is None:
            errors.append(f"{label} sha256 must be lowercase sha256 hex")
            digest = None
        elif digest == "0" * 64:
            errors.append(f"{label} sha256 must be non-zero lowercase sha256 hex")
            digest = None
    if relative is not None:
        relative = _normalise_safe_relative_path(
            relative,
            errors,
            f"{label} path",
        )
    if relative is None:
        return None, None
    if not _safe_relative_path_is_child_of(relative, "handoff"):
        errors.append(f"{label} path must stay under handoff/")
        return None, None
    _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
        slot_path,
        relative,
        f"{label} path",
        f"{label} path must point to an existing file",
    )
    if digest_errors:
        errors.extend(digest_errors)
        return None, None
    if digest is None or actual_digest is None:
        return None, None
    if actual_digest != digest:
        errors.append(f"{label} sha256 does not match path")
        return None, None
    transcript_transport = validate_d2d_payment_transcript(
        slot_path,
        slot_path / relative,
        metadata,
        errors,
    )
    if transcript_transport != transport_key:
        errors.append(f"{label} transport must match transcript transport")
        return None, None
    return transport_key, {"path": relative, "sha256": digest}


def validate_d2d_payment_transcripts_binding(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
    *,
    primary_relative: str | None,
    primary_digest: str | None,
    primary_transport: str | None,
) -> dict[str, dict[str, str]]:
    """Validate optional per-transport D2D transcript bindings."""

    transcripts: dict[str, dict[str, str]] = {}
    if (
        primary_relative is not None
        and primary_digest is not None
        and primary_transport is not None
    ):
        transcripts[primary_transport] = {
            "path": primary_relative,
            "sha256": primary_digest,
        }
    value = metadata.get(D2D_PAYMENT_TRANSCRIPTS_FIELD)
    if value is None:
        return transcripts
    if not isinstance(value, dict) or not value:
        errors.append(f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} must be a non-empty object")
        return transcripts
    seen_paths: dict[str, str] = {}
    for raw_transport, entry in sorted(value.items()):
        if not isinstance(raw_transport, str) or raw_transport not in D2D_PAYMENT_TRANSPORTS:
            errors.append(
                f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} keys must be one of "
                f"{sorted(D2D_PAYMENT_TRANSPORTS)}"
            )
            continue
        transport, validated = _validate_d2d_payment_transcript_entry(
            slot_path,
            metadata,
            raw_transport,
            entry,
            errors,
        )
        if transport is None or validated is None:
            continue
        previous_transport = seen_paths.get(validated["path"])
        if previous_transport is not None and previous_transport != transport:
            errors.append(
                f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} must not reuse "
                f"{validated['path']} for multiple transports"
            )
            continue
        seen_paths[validated["path"]] = transport
        transcripts[transport] = validated
    if (
        primary_transport is not None
        and primary_transport not in transcripts
    ):
        errors.append(
            f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} must include the primary "
            "d2d_payment_transcript_path transport"
        )
    return transcripts


def validate_wallet_integrity_transcript(
    transcript_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate one-use-key rotation and rollback-rejection transcript evidence."""

    transcript = _load_json(transcript_path, "wallet integrity transcript", errors)
    if transcript is None:
        return

    for field in sorted(set(transcript) - WALLET_INTEGRITY_TRANSCRIPT_FIELDS):
        errors.append(
            f"wallet integrity transcript contains unexpected field {_display_path(field)}"
        )

    if transcript.get("schema") != WALLET_INTEGRITY_TRANSCRIPT_SCHEMA:
        errors.append(
            f"wallet integrity transcript schema must be {WALLET_INTEGRITY_TRANSCRIPT_SCHEMA}"
        )

    for key in WALLET_INTEGRITY_TRANSCRIPT_SLOT_STRING_BINDINGS:
        actual = _wallet_transcript_string(transcript, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and actual is not None and actual != expected:
            errors.append(f"wallet integrity transcript {key} must match slot.json {key}")

    for key in WALLET_INTEGRITY_TRANSCRIPT_SLOT_SHA256_BINDINGS:
        actual = _wallet_transcript_sha256(transcript, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and actual is not None and actual != expected:
            errors.append(f"wallet integrity transcript {key} must match slot.json {key}")

    for key in WALLET_INTEGRITY_TRANSCRIPT_TRUE_FIELDS:
        _wallet_transcript_true(transcript, key, errors)
        if metadata.get(key) is not None and transcript.get(key) != metadata.get(key):
            errors.append(f"wallet integrity transcript {key} must match slot.json {key}")

    _wallet_transcript_sha256(transcript, "rotation_session_id_sha256", errors)
    _require_distinct_wallet_digests(
        transcript,
        "key_id_before_sha256",
        "key_id_after_sha256",
        errors,
    )
    _require_distinct_wallet_digests(
        transcript,
        "wallet_state_before_sha256",
        "wallet_state_after_rotation_sha256",
        errors,
    )
    _require_distinct_wallet_digests(
        transcript,
        "rollback_snapshot_sha256",
        "wallet_state_after_rotation_sha256",
        errors,
    )
    _wallet_transcript_sha256(transcript, "restored_snapshot_sha256", errors)


def validate_wallet_integrity_transcript_binding(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> tuple[str | None, str | None]:
    """Validate slot.json path/hash binding for the wallet integrity transcript."""

    if _reject_secret_slot_path(slot_path, errors):
        return None, None
    digest = _require_lowercase_sha256_hex(
        metadata,
        "wallet_integrity_transcript_sha256",
        "slot.json",
        errors,
    )
    relative = _require_non_empty_string(
        metadata,
        "wallet_integrity_transcript_path",
        errors,
    )
    if relative is not None:
        relative = _normalise_safe_relative_path(
            relative,
            errors,
            "slot.json wallet_integrity_transcript_path",
        )
    if relative is None:
        return None, digest
    if not _safe_relative_path_is_child_of(relative, "wallet"):
        errors.append("slot.json wallet_integrity_transcript_path must stay under wallet/")
        return relative, digest

    _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
        slot_path,
        relative,
        "slot.json wallet_integrity_transcript_path",
        "slot.json wallet_integrity_transcript_path must point to an existing file",
    )
    if digest_errors:
        errors.extend(digest_errors)
        return relative, None

    matched_digest: str | None = None
    if digest is not None and actual_digest is not None:
        if actual_digest != digest:
            errors.append(
                "slot.json wallet_integrity_transcript_sha256 does not match wallet_integrity_transcript_path"
            )
        else:
            matched_digest = digest
    transcript_path = slot_path / relative
    validate_wallet_integrity_transcript(transcript_path, metadata, errors)
    return relative, matched_digest


def _require_lowercase_sha256_hex(
    data: dict[str, Any], key: str, label: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"{label} {key} must be lowercase sha256 hex")
        return None
    if value == "0" * 64:
        errors.append(f"{label} {key} must be non-zero lowercase sha256 hex")
        return None
    return value


def _validate_attestation_certificate_chain_artifact(
    relative: str,
    payload: bytes,
    errors: list[str],
) -> None:
    suffix = PurePosixPath(relative).suffix.lower()
    if suffix not in ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES:
        errors.append(
            "slot.json attestation_certificate_chain_path must end in .pem or .der"
        )
    if not payload:
        errors.append("attestation certificate chain must be non-empty")
        return
    if len(payload) > MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES:
        errors.append(
            "attestation certificate chain must be no more than "
            f"{MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES} bytes"
        )
    if suffix == ".pem":
        if (
            b"-----BEGIN CERTIFICATE-----" not in payload
            or b"-----END CERTIFICATE-----" not in payload
        ):
            errors.append(
                "attestation certificate chain PEM must contain certificate boundaries"
            )
    elif suffix == ".der" and not payload.startswith(b"\x30"):
        errors.append("attestation certificate chain DER must start with ASN.1 SEQUENCE")


def _canonical_signed_evidence_payload(evidence: dict[str, Any]) -> bytes:
    payload = {
        key: value
        for key, value in evidence.items()
        if key not in {"signature", "signature_payload_sha256"}
    }
    return json.dumps(
        payload,
        sort_keys=True,
        separators=(",", ":"),
        allow_nan=False,
    ).encode("utf-8")


def _parse_hex_bytes(
    value: str | None,
    *,
    expected_len: int,
    label: str,
    errors: list[str],
) -> bytes | None:
    if value is None:
        return None
    if (
        len(value) != expected_len * 2
        or not re.fullmatch(r"[0-9a-f]+", value)
    ):
        errors.append(f"{label} must be {expected_len} lowercase hex bytes")
        return None
    return bytes.fromhex(value)


def _require_openssl(errors: list[str]) -> str | None:
    openssl = shutil.which("openssl")
    if openssl is None:
        errors.append("openssl is required to verify Kagemusha signed evidence artifacts")
        return None
    return openssl


def _openssl_public_key_der(
    public_key_path: Path,
    *,
    errors: list[str],
    label: str,
) -> bytes | None:
    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):
        return None
    try:
        public_key_bytes = public_key_path.read_bytes()
    except OSError:
        errors.append(f"{label} file could not be read")
        return None
    if any(marker in public_key_bytes for marker in PRIVATE_KEY_PEM_MARKERS):
        errors.append(f"{label} must contain public key material, not a private key")
        return None
    openssl = _require_openssl(errors)
    if openssl is None:
        return None
    try:
        completed = subprocess.run(
            [
                openssl,
                "pkey",
                "-pubin",
                "-in",
                str(public_key_path),
                "-pubout",
                "-outform",
                "DER",
            ],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except subprocess.CalledProcessError:
        errors.append(f"{label} must be a valid OpenSSL public key")
        return None
    except OSError:
        errors.append(f"{label} OpenSSL public key command could not be run")
        return None
    return completed.stdout


def _validate_public_key_path_shape(
    public_key_path: Path,
    *,
    errors: list[str],
    label: str,
) -> bool:
    """Reject public key paths that could alias external key material."""

    path_text = str(public_key_path)
    if SECRET_RE.search(path_text):
        errors.append(f"{label} path must not contain secret-looking material")
        return False
    if _contains_control_character(path_text):
        errors.append(f"{label} path must not contain control characters")
        return False
    if path_text != path_text.strip() or _path_has_surrounding_whitespace_component(
        public_key_path
    ):
        errors.append(f"{label} path must not contain surrounding whitespace")
        return False
    if "\\" in path_text:
        errors.append(f"{label} path must not contain backslashes")
        return False
    if ".." in public_key_path.parts:
        errors.append(f"{label} path must be canonical")
        return False
    try:
        public_key_mode = public_key_path.lstat().st_mode
    except FileNotFoundError:
        public_key_mode = None
    except OSError:
        errors.append(f"{label} file metadata could not be read")
        return False
    if public_key_mode is not None and stat.S_ISLNK(public_key_mode):
        errors.append(f"{label} must not be a symlink")
        return False
    ancestor_errors = validate_no_symlink_ancestors(
        public_key_path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return False
    if public_key_mode is None:
        errors.append(f"{label} must point to an existing public key file")
        return False
    if not stat.S_ISREG(public_key_mode):
        errors.append(f"{label} must be a regular file")
        return False
    try:
        link_count = public_key_path.stat().st_nlink
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return False
    if link_count > 1:
        errors.append(f"{label} must not be hardlinked")
        return False
    return True


def _trusted_signer_public_key_path_inputs(
    public_key_paths: Iterable[str | os.PathLike[str]] | str | os.PathLike[str] | None,
) -> tuple[tuple[Any, ...], list[str]]:
    """Normalize direct trusted-signer path inputs without iterating path strings."""

    if public_key_paths is None:
        return (), []
    if isinstance(public_key_paths, (str, bytes, bytearray, os.PathLike)):
        return (public_key_paths,), []
    try:
        return tuple(public_key_paths), []
    except TypeError:
        return (), ["trusted signer public key paths must be an iterable of paths"]


def load_trusted_signer_public_keys(
    public_key_paths: Iterable[str | os.PathLike[str]] | str | os.PathLike[str] | None,
) -> tuple[dict[str, Path], list[str]]:
    """Load trusted lab signer public keys and return them keyed by DER SHA-256."""

    raw_paths, errors = _trusted_signer_public_key_path_inputs(public_key_paths)
    trusted: dict[str, Path] = {}
    for raw_path in raw_paths:
        try:
            path = Path(raw_path)
        except TypeError:
            errors.append("trusted signer public key path must be a string or pathlib Path")
            continue
        der = _openssl_public_key_der(
            path,
            errors=errors,
            label="trusted signer public key",
        )
        if der is None:
            continue
        digest = hashlib.sha256(der).hexdigest()
        if digest in trusted:
            errors.append("duplicate trusted signer public key")
            continue
        trusted[digest] = path
    return trusted, errors


def _valid_trusted_signer_public_key_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and SHA256_HEX_RE.fullmatch(value) is not None
        and value != "0" * 64
    )


def _trusted_signer_public_key_sha256_set(
    trusted_signer_public_keys: Mapping[Any, Any] | None,
) -> frozenset[str]:
    if not isinstance(trusted_signer_public_keys, Mapping):
        return frozenset()
    return frozenset(
        digest
        for digest in trusted_signer_public_keys
        if _valid_trusted_signer_public_key_sha256(digest)
    )


def _trusted_signer_digest_sort_key(item: tuple[Any, Any]) -> tuple[int, str, str, int]:
    digest = item[0]
    if isinstance(digest, str):
        return (0, digest, "", 0)
    digest_type = type(digest)
    return (1, digest_type.__module__, digest_type.__qualname__, id(digest))


def validate_trusted_signer_public_key_map(
    trusted_signer_public_keys: Mapping[Any, Any] | None,
) -> list[str]:
    """Reject direct trusted-signer maps with unsafe or misbound public keys."""

    errors: list[str] = []
    if trusted_signer_public_keys is None:
        return errors
    if not isinstance(trusted_signer_public_keys, Mapping):
        return ["trusted signer public key map must be a mapping"]
    for digest, public_key_path in sorted(
        trusted_signer_public_keys.items(),
        key=_trusted_signer_digest_sort_key,
    ):
        if not _valid_trusted_signer_public_key_sha256(digest):
            errors.append("trusted signer public key digest must be non-zero lowercase sha256 hex")
            continue
        if not isinstance(public_key_path, Path):
            errors.append("trusted signer public key path must be a pathlib Path")
            continue
        path_text = str(public_key_path)
        if SECRET_RE.search(path_text):
            errors.append("trusted signer public key path must not contain secret-looking material")
            continue
        if _contains_control_character(path_text):
            errors.append("trusted signer public key path must not contain control characters")
            continue
        if path_text != path_text.strip() or _path_has_surrounding_whitespace_component(
            public_key_path
        ):
            errors.append("trusted signer public key path must not contain surrounding whitespace")
            continue
        if "\\" in path_text:
            errors.append("trusted signer public key path must not contain backslashes")
            continue
        if ".." in public_key_path.parts:
            errors.append("trusted signer public key path must be canonical")
            continue
        der = _openssl_public_key_der(
            public_key_path,
            errors=errors,
            label="trusted signer public key",
        )
        if der is None:
            continue
        actual_digest = hashlib.sha256(der).hexdigest()
        if actual_digest != digest:
            errors.append(
                "trusted signer public key digest must match public key DER sha256"
            )
    return errors


def _write_staged_bytes(
    path: Path,
    payload: bytes,
    *,
    write_error: str,
    verification_error: str,
) -> list[str]:
    """Write OpenSSL staging bytes durably enough for immediate subprocess use."""

    staged_stat: os.stat_result | None = None
    try:
        with path.open("xb") as handle:
            handle.write(payload)
            handle.flush()
            os.fsync(handle.fileno())
            staged_stat = os.fstat(handle.fileno())
    except OSError:
        return [write_error]
    assert staged_stat is not None
    readback, readback_errors = _read_staged_bytes(
        path,
        staged_stat,
        verification_error,
    )
    if readback_errors:
        return readback_errors
    if readback != payload:
        return [verification_error]
    return []


def _read_staged_bytes(
    path: Path,
    expected_stat: os.stat_result,
    verification_error: str,
) -> tuple[bytes | None, list[str]]:
    """Read staged bytes without accepting a swapped staging path."""

    chunks: list[bytes] = []
    staged_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, [verification_error]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, [verification_error]
            staged_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if staged_open_identity != staged_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != staged_expected_identity:
                return None, [verification_error]
            if open_stat.st_nlink > 1:
                return None, [verification_error]
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != staged_expected_identity:
                return None, [verification_error]
    except OSError:
        return None, [verification_error]
    return b"".join(chunks), []


def _verify_ed25519_signature(
    *,
    public_key_path: Path,
    payload: bytes,
    signature: bytes,
    errors: list[str],
    label: str = "trusted signer public key",
) -> None:
    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):
        return
    openssl = _require_openssl(errors)
    if openssl is None:
        return
    try:
        with tempfile.TemporaryDirectory(prefix="iroha-kagemusha-evidence-") as temp:
            temp_path = Path(temp)
            payload_path = temp_path / "payload.bin"
            signature_path = temp_path / "signature.bin"
            stage_errors = _write_staged_bytes(
                payload_path,
                payload,
                write_error="signature verification staging files could not be written",
                verification_error="signature verification staged payload did not match input",
            )
            if stage_errors:
                errors.extend(stage_errors)
                return
            stage_errors = _write_staged_bytes(
                signature_path,
                signature,
                write_error="signature verification staging files could not be written",
                verification_error="signature verification staged signature did not match input",
            )
            if stage_errors:
                errors.extend(stage_errors)
                return
            try:
                completed = subprocess.run(
                    [
                        openssl,
                        "pkeyutl",
                        "-verify",
                        "-pubin",
                        "-inkey",
                        str(public_key_path),
                        "-rawin",
                        "-in",
                        str(payload_path),
                        "-sigfile",
                        str(signature_path),
                    ],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.PIPE,
                )
            except OSError:
                errors.append("signature verification command could not be run")
                return
    except OSError:
        errors.append("signature verification temporary directory could not be created")
        return
    if completed.returncode != 0:
        errors.append("signed evidence artifact signature verification failed")


def _validate_signed_at_utc(value: str | None, errors: list[str]) -> None:
    if value is None:
        return
    if not SIGNED_AT_UTC_RE.fullmatch(value):
        errors.append(
            "signed evidence artifact signed_at_utc must be canonical UTC YYYY-MM-DDTHH:MM:SSZ"
        )
        return
    try:
        dt.datetime.strptime(value, "%Y-%m-%dT%H:%M:%SZ").replace(
            tzinfo=dt.timezone.utc
        )
    except ValueError:
        errors.append(
            "signed evidence artifact signed_at_utc must be a valid UTC timestamp"
        )


def _is_required_signed_evidence_digest_path(relative: str) -> bool:
    for root in (*EXPECTED_DIRS, "handoff", "wallet"):
        if _safe_relative_path_is_child_of(relative, root):
            return True
    return (
        _safe_relative_path_is_child_of(relative, "evidence")
        and relative != KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH
    )


def _required_signed_evidence_digest_paths(
    slot_path: Path,
    errors: list[str] | None = None,
    metadata: dict[str, Any] | None = None,
) -> list[str]:
    paths = {
        relative
        for relative in _slot_files(slot_path, errors)
        if _is_required_signed_evidence_digest_path(relative)
    } | set(REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS)
    if metadata is not None:
        path_errors = errors if errors is not None else []
        for field in SIGNED_EVIDENCE_SLOT_ARTIFACT_PATH_FIELDS:
            value = metadata.get(field)
            if isinstance(value, str):
                relative = _normalise_safe_relative_path(
                    value,
                    path_errors,
                    f"slot.json {field}",
                )
                if relative is not None:
                    paths.add(relative)
        transcript_map = metadata.get(D2D_PAYMENT_TRANSCRIPTS_FIELD)
        if isinstance(transcript_map, dict):
            for raw_entry in transcript_map.values():
                if not isinstance(raw_entry, dict):
                    continue
                value = raw_entry.get("path")
                if isinstance(value, str):
                    relative = _normalise_safe_relative_path(
                        value,
                        path_errors,
                        f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} path",
                    )
                    if relative is not None:
                        paths.add(relative)
    return sorted(paths)


def validate_required_kagemusha_slot_artifact_shapes(
    slot_path: Path,
    errors: list[str],
    expected_app_package_name: str | None = None,
    expected_app_package_label: str = "slot.json app_package_name",
    expected_device_model: str | None = None,
    expected_device_codename: str | None = None,
) -> None:
    """Validate base production slot artifacts before they are signed or accepted."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS:
        artifact_path = slot_path / relative
        mode, mode_errors = _slot_artifact_lstat_mode(
            artifact_path,
            f"required slot artifact metadata could not be read {relative}",
        )
        if mode_errors:
            errors.extend(mode_errors)
            continue
        if mode is None or stat.S_ISLNK(mode) or not stat.S_ISREG(mode):
            continue
        try:
            artifact_size = artifact_path.stat().st_size
        except OSError:
            errors.append(f"required slot artifact metadata could not be read {relative}")
            continue
        if artifact_size == 0:
            errors.append(f"required slot artifact {relative} must be non-empty")
        elif artifact_size > MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES:
            errors.append(
                f"required slot artifact {relative} must be no more than "
                f"{MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES} bytes"
            )

    _validate_required_pending_queue_artifact(slot_path, errors)
    _validate_required_telemetry_artifact(
        slot_path,
        errors,
        expected_app_package_name=expected_app_package_name,
        expected_app_package_label=expected_app_package_label,
        expected_device_model=expected_device_model,
        expected_device_codename=expected_device_codename,
    )
    _validate_required_status_artifact(slot_path, errors)
    _validate_required_runtime_log_artifact(slot_path, errors)


def _validate_required_pending_queue_artifact(slot_path: Path, errors: list[str]) -> None:
    queue = _load_json(
        slot_path / "queue" / "pending_queue.json",
        "queue/pending_queue.json",
        errors,
    )
    if queue is None:
        return
    for field in sorted(set(queue) - PENDING_QUEUE_FIELDS):
        errors.append(
            f"queue/pending_queue.json contains unexpected field {_display_path(field)}"
        )
    slot_id = queue.get("slot_id")
    if not isinstance(slot_id, str) or not slot_id:
        errors.append("queue/pending_queue.json slot_id must be a non-empty string")
    elif slot_id != slot_id.strip():
        errors.append("queue/pending_queue.json slot_id must not contain surrounding whitespace")
    elif _contains_control_character(slot_id):
        errors.append("queue/pending_queue.json slot_id must not contain control characters")
    elif slot_id != slot_path.name:
        errors.append("queue/pending_queue.json slot_id must match slot id")
    pending_transactions = queue.get("pending_transactions")
    if not isinstance(pending_transactions, list):
        errors.append("queue/pending_queue.json pending_transactions must be an array")
    elif pending_transactions:
        errors.append(
            "queue/pending_queue.json pending_transactions must be empty after D2D handoff"
        )


def _validate_telemetry_string(
    telemetry: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = telemetry.get(key)
    label = f"telemetry/telemetry.json {key}"
    if not isinstance(value, str) or not value:
        errors.append(f"{label} must be a non-empty string")
        return None
    if value != value.strip():
        errors.append(f"{label} must not contain surrounding whitespace")
        return None
    if _contains_control_character(value):
        errors.append(f"{label} must not contain control characters")
        return None
    if SECRET_RE.search(value):
        errors.append(f"{label} must not contain secret-looking material")
        return None
    return value


def _validate_required_telemetry_artifact(
    slot_path: Path,
    errors: list[str],
    expected_app_package_name: str | None = None,
    expected_app_package_label: str = "slot.json app_package_name",
    expected_device_model: str | None = None,
    expected_device_codename: str | None = None,
) -> None:
    telemetry = _load_json(
        slot_path / "telemetry" / "telemetry.json",
        "telemetry/telemetry.json",
        errors,
    )
    if telemetry is None:
        return
    for field in sorted(set(telemetry) - TELEMETRY_FIELDS):
        errors.append(
            f"telemetry/telemetry.json contains unexpected field {_display_path(field)}"
        )
    if telemetry.get("schema_version") != 1:
        errors.append("telemetry/telemetry.json schema_version must be 1")
    slot_id = telemetry.get("slot_id")
    if not isinstance(slot_id, str) or not slot_id:
        errors.append("telemetry/telemetry.json slot_id must be a non-empty string")
    elif slot_id != slot_id.strip():
        errors.append("telemetry/telemetry.json slot_id must not contain surrounding whitespace")
    elif _contains_control_character(slot_id):
        errors.append("telemetry/telemetry.json slot_id must not contain control characters")
    elif slot_id != slot_path.name:
        errors.append("telemetry/telemetry.json slot_id must match the slot directory name")
    suite = telemetry.get("suite")
    if not isinstance(suite, str) or not suite:
        errors.append("telemetry/telemetry.json suite must be a non-empty string")
    elif suite != suite.strip():
        errors.append("telemetry/telemetry.json suite must not contain surrounding whitespace")
    elif _contains_control_character(suite):
        errors.append("telemetry/telemetry.json suite must not contain control characters")
    elif suite != KAGEMUSHA_TELEMETRY_SUITE:
        errors.append("telemetry/telemetry.json suite must identify a Kagemusha device-lab run")
    telemetry_strings: dict[str, str] = {}
    for key in TELEMETRY_STRING_FIELDS:
        value = _validate_telemetry_string(telemetry, key, errors)
        if value is not None:
            telemetry_strings[key] = value
    app_package_name = telemetry_strings.get("app_package_name")
    if (
        expected_app_package_name is not None
        and app_package_name is not None
        and app_package_name != expected_app_package_name
    ):
        errors.append(
            "telemetry/telemetry.json app_package_name must match "
            f"{expected_app_package_label}"
        )
    for key, expected in (
        ("device_model", expected_device_model),
        ("device_codename", expected_device_codename),
    ):
        value = telemetry_strings.get(key)
        if expected is not None and value is not None and value != expected:
            errors.append(f"telemetry/telemetry.json {key} must match slot.json {key}")


def _validate_required_status_artifact(slot_path: Path, errors: list[str]) -> None:
    if not _should_read_optional_text_artifact(
        slot_path,
        "telemetry/status.ndjson",
        "telemetry/status.ndjson",
        errors,
    ):
        return
    text, read_errors = _metadata_artifact_text(
        slot_path,
        "telemetry/status.ndjson",
        "telemetry/status.ndjson",
        "telemetry/status.ndjson required artifact is missing",
        "telemetry/status.ndjson could not be read",
    )
    if read_errors:
        errors.extend(read_errors)
        return
    assert text is not None
    if "\r" in text:
        errors.append("telemetry/status.ndjson must use LF line endings")
    if text and not text.endswith("\n"):
        errors.append("telemetry/status.ndjson must end with a trailing newline")
    lines = text.splitlines()

    saw_record = False
    saw_ok = False
    for line_no, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()
        if not line:
            continue
        saw_record = True
        if raw_line != line:
            errors.append(
                f"telemetry/status.ndjson line {line_no} must not contain surrounding whitespace"
            )
            continue
        try:
            status_entry = _loads_json_without_duplicate_keys(line)
        except json.JSONDecodeError as exc:
            errors.append(f"telemetry/status.ndjson line {line_no} is not valid JSON: {exc}")
            continue
        except DuplicateJsonKeyError as exc:
            errors.append(
                "telemetry/status.ndjson line "
                f"{line_no} contains duplicate JSON object key {_display_path(exc.key)}"
            )
            continue
        if not isinstance(status_entry, dict):
            errors.append(f"telemetry/status.ndjson line {line_no} must be a JSON object")
            continue
        for field in sorted(set(status_entry) - STATUS_EVENT_FIELDS):
            errors.append(
                f"telemetry/status.ndjson line {line_no} contains unexpected field {_display_path(field)}"
            )
        status = status_entry.get("status")
        if not isinstance(status, str) or not status:
            errors.append(f"telemetry/status.ndjson line {line_no} status must be a non-empty string")
            continue
        if status != status.strip():
            errors.append(f"telemetry/status.ndjson line {line_no} status must not contain surrounding whitespace")
            continue
        if _contains_control_character(status):
            errors.append(f"telemetry/status.ndjson line {line_no} status must not contain control characters")
            continue
        if status != status.lower():
            errors.append(f"telemetry/status.ndjson line {line_no} status must be lowercase")
            continue
        slot_value = status_entry.get("slot_id")
        if slot_value is None:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a non-empty string")
        elif not isinstance(slot_value, str):
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a string")
        elif not slot_value:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must be a non-empty string")
        elif isinstance(slot_value, str) and slot_value != slot_value.strip():
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must not contain surrounding whitespace")
        elif isinstance(slot_value, str) and _contains_control_character(slot_value):
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must not contain control characters")
        elif slot_value != slot_path.name:
            errors.append(f"telemetry/status.ndjson line {line_no} slot_id must match slot id")
        if status == "ok":
            saw_ok = True
        elif status in KAGEMUSHA_STATUS_FAILURE_VALUES:
            errors.append(
                f"telemetry/status.ndjson line {line_no} status must not be {status!r}"
            )
        else:
            errors.append(f"telemetry/status.ndjson line {line_no} status must be ok")

    if not saw_record:
        errors.append("telemetry/status.ndjson must contain at least one JSON status record")
    elif not saw_ok:
        errors.append("telemetry/status.ndjson must contain at least one ok status")


def _validate_required_runtime_log_artifact(slot_path: Path, errors: list[str]) -> None:
    if not _should_read_optional_text_artifact(
        slot_path,
        "logs/runtime.log",
        "logs/runtime.log",
        errors,
    ):
        return
    text, read_errors = _metadata_artifact_text(
        slot_path,
        "logs/runtime.log",
        "logs/runtime.log",
        "logs/runtime.log required artifact is missing",
        "logs/runtime.log could not be read",
        decode_errors="replace",
    )
    if read_errors:
        errors.extend(read_errors)
        return
    assert text is not None
    if KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER not in text:
        errors.append("logs/runtime.log must contain Kagemusha device-lab completion marker")
    for marker in KAGEMUSHA_RUNTIME_LOG_FAILURE_MARKERS:
        if marker in text:
            errors.append(f"logs/runtime.log must not contain failure marker {marker}")


def validate_signed_evidence_artifact(
    slot_path: Path,
    artifact_path: Path,
    metadata: dict[str, Any],
    trusted_signer_public_keys: dict[str, Path],
    errors: list[str],
) -> dict[str, Any]:
    """Validate the structured signed lab-evidence artifact."""

    details: dict[str, Any] = {}
    if _reject_secret_slot_path(slot_path, errors):
        return details
    evidence = _load_json(artifact_path, "signed evidence artifact", errors)
    if evidence is None:
        return details

    unexpected_fields = sorted(set(evidence) - SIGNED_EVIDENCE_FIELDS)
    for field in unexpected_fields:
        errors.append(
            f"signed evidence artifact contains unexpected field {_display_path(field)}"
        )

    if evidence.get("schema") != SIGNED_EVIDENCE_SCHEMA:
        errors.append(f"signed evidence artifact schema must be {SIGNED_EVIDENCE_SCHEMA}")

    for key in SIGNED_EVIDENCE_SLOT_STRING_FIELDS:
        value = _require_evidence_string(evidence, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and value is not None and value != expected:
            errors.append(f"signed evidence artifact {key} must match slot.json {key}")
    for key in SIGNED_EVIDENCE_SLOT_SHA256_FIELDS:
        value = _require_lowercase_sha256_hex(
            evidence,
            key,
            "signed evidence artifact",
            errors,
        )
        expected = metadata.get(key)
        if isinstance(expected, str) and value is not None and value != expected:
            errors.append(f"signed evidence artifact {key} must match slot.json {key}")
    for key in SIGNED_EVIDENCE_SLOT_INT_FIELDS:
        value = _require_evidence_int(evidence, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, int) and not isinstance(expected, bool) and value != expected:
            errors.append(f"signed evidence artifact {key} must match slot.json {key}")
    for key in SIGNED_EVIDENCE_SLOT_TRUE_FIELDS:
        _require_evidence_true(evidence, key, errors)
        if metadata.get(key) is not None and evidence.get(key) != metadata.get(key):
            errors.append(f"signed evidence artifact {key} must match slot.json {key}")
    if (
        D2D_PAYMENT_TRANSCRIPTS_FIELD in metadata
        or D2D_PAYMENT_TRANSCRIPTS_FIELD in evidence
    ) and evidence.get(D2D_PAYMENT_TRANSCRIPTS_FIELD) != metadata.get(
        D2D_PAYMENT_TRANSCRIPTS_FIELD
    ):
        errors.append(
            "signed evidence artifact d2d_payment_transcripts must match "
            "slot.json d2d_payment_transcripts"
        )

    evidence_commands = evidence.get("raw_test_commands")
    metadata_commands = metadata.get("raw_test_commands")
    if not isinstance(evidence_commands, list) or not evidence_commands:
        errors.append("signed evidence artifact raw_test_commands must be a non-empty array")
    else:
        for index, command in enumerate(evidence_commands):
            if not isinstance(command, str) or not command.strip():
                errors.append(
                    f"signed evidence artifact raw_test_commands[{index}] must be a non-empty string"
                )
                continue
            if command != command.strip():
                errors.append(
                    f"signed evidence artifact raw_test_commands[{index}] must not contain surrounding whitespace"
                )
                continue
            if _contains_control_character(command):
                errors.append(
                    f"signed evidence artifact raw_test_commands[{index}] must not contain control characters"
                )
                continue
            if SECRET_RE.search(command):
                errors.append(
                    f"signed evidence artifact raw_test_commands[{index}] must not contain secret-looking material"
                )
                continue
        _validate_raw_test_command_markers(
            evidence_commands,
            label="signed evidence artifact raw_test_commands",
            errors=errors,
        )
        if isinstance(metadata_commands, list) and evidence_commands != metadata_commands:
            errors.append(
                "signed evidence artifact raw_test_commands must match slot.json raw_test_commands"
            )

    signed_at = _require_evidence_raw_string(evidence, "signed_at_utc", errors)
    _validate_signed_at_utc(signed_at, errors)
    if signed_at is not None:
        details["signed_at_utc"] = signed_at
    _require_evidence_string(evidence, "signer_key_id", errors)
    signer_public_key_sha256 = _require_lowercase_sha256_hex(
        evidence,
        "signer_public_key_sha256",
        "signed evidence artifact",
        errors,
    )
    if signer_public_key_sha256 is not None:
        details["signer_public_key_sha256"] = signer_public_key_sha256
    algorithm = _require_evidence_string(evidence, "signature_algorithm", errors)
    if algorithm is not None and algorithm not in SIGNED_EVIDENCE_SIGNATURE_ALGORITHMS:
        errors.append(
            "signed evidence artifact signature_algorithm must be one of "
            f"{sorted(SIGNED_EVIDENCE_SIGNATURE_ALGORITHMS)}"
        )
    signature_text = _require_evidence_string(evidence, "signature", errors)
    signature = _parse_hex_bytes(
        signature_text,
        expected_len=ED25519_SIGNATURE_BYTES,
        label="signed evidence artifact signature",
        errors=errors,
    )

    try:
        payload = _canonical_signed_evidence_payload(evidence)
    except ValueError:
        errors.append("signed evidence artifact signature payload is not strict JSON")
        return details
    expected_payload_digest = hashlib.sha256(payload).hexdigest()
    payload_digest = _require_lowercase_sha256_hex(
        evidence,
        "signature_payload_sha256",
        "signed evidence artifact",
        errors,
    )
    if payload_digest is not None and payload_digest != expected_payload_digest:
        errors.append("signed evidence artifact signature_payload_sha256 mismatch")

    digests = evidence.get("artifact_digests")
    if not isinstance(digests, dict) or not digests:
        errors.append("signed evidence artifact artifact_digests must be a non-empty object")
        return details

    expected_app_package_name = metadata.get("app_package_name")
    expected_device_model = metadata.get("device_model")
    expected_device_codename = metadata.get("device_codename")
    validate_required_kagemusha_slot_artifact_shapes(
        slot_path,
        errors,
        expected_app_package_name=(
            expected_app_package_name if isinstance(expected_app_package_name, str) else None
        ),
        expected_app_package_label="slot.json app_package_name",
        expected_device_model=(
            expected_device_model if isinstance(expected_device_model, str) else None
        ),
        expected_device_codename=(
            expected_device_codename if isinstance(expected_device_codename, str) else None
        ),
    )

    required_paths = _required_signed_evidence_digest_paths(slot_path, errors, metadata)
    required_path_set = set(required_paths)
    for raw_relative in digests:
        if not isinstance(raw_relative, str):
            errors.append("signed evidence artifact artifact_digests keys must be strings")
            continue
        if SECRET_RE.search(raw_relative):
            errors.append(
                "signed evidence artifact artifact_digests keys must not contain secret-looking material"
            )
            continue
        relative = _normalise_safe_relative_path(
            raw_relative,
            errors,
            "signed evidence artifact artifact_digests",
        )
        if relative is not None and relative not in required_path_set:
            errors.append(
                "signed evidence artifact artifact_digests contains unexpected path "
                f"{_display_path(relative)}"
            )

    for relative in required_paths:
        digest = digests.get(relative)
        if not isinstance(digest, str) or not SHA256_HEX_RE.fullmatch(digest):
            errors.append(
                "signed evidence artifact artifact_digests"
                f"[{_display_path(relative)}] must be lowercase sha256 hex"
            )
            continue
        if digest == "0" * 64:
            errors.append(
                "signed evidence artifact artifact_digests"
                f"[{_display_path(relative)}] must be non-zero lowercase sha256 hex"
            )
            continue
        actual_digest, digest_errors = _signed_evidence_artifact_sha256(
            slot_path,
            relative,
        )
        if digest_errors:
            errors.extend(digest_errors)
            continue
        assert actual_digest is not None
        if digest != actual_digest:
            errors.append(
                f"signed evidence artifact digest mismatch for {_display_path(relative)}"
            )

    if not trusted_signer_public_keys:
        errors.append("trusted signer public key required for Kagemusha production evidence")
        return details
    if signer_public_key_sha256 is None or signature is None:
        return details
    trusted_public_key = trusted_signer_public_keys.get(signer_public_key_sha256)
    if trusted_public_key is None:
        errors.append(
            "signed evidence artifact signer_public_key_sha256 must match a trusted signer public key"
        )
        return details
    if algorithm == "ed25519":
        _verify_ed25519_signature(
            public_key_path=trusted_public_key,
            payload=payload,
            signature=signature,
            errors=errors,
        )
    return details


def validate_kagemusha_production_metadata(
    slot_path: Path,
    trusted_signer_public_keys: dict[str, Path] | None = None,
) -> tuple[list[str], dict[str, Any]]:
    """Validate production Kagemusha Android lab evidence metadata."""

    trusted_signer_public_keys = trusted_signer_public_keys or {}
    errors: list[str] = []
    details: dict[str, Any] = {}
    signer_map_errors = validate_trusted_signer_public_key_map(
        trusted_signer_public_keys
    )
    if signer_map_errors:
        return signer_map_errors, details
    if _reject_secret_slot_path(slot_path, errors):
        return errors, details
    metadata = _load_json(slot_path / "slot.json", "slot.json", errors)
    if metadata is None:
        return errors, details

    attestation_certificate_chain_bytes: bytes | None = None
    validate_slot_metadata_fields(metadata, errors)
    if metadata.get("schema") != "iroha.android.device_lab.kagemusha.v1":
        errors.append("slot.json schema must be iroha.android.device_lab.kagemusha.v1")
    slot_id = _require_non_empty_string(metadata, "slot_id", errors)
    if slot_id is not None and slot_id != slot_path.name:
        errors.append("slot.json slot_id must match the slot directory name")
    family = _require_non_empty_string(metadata, "device_family", errors)
    details["device_family"] = family
    device_fingerprint = _require_non_empty_string(metadata, "device_fingerprint", errors)
    if device_fingerprint is not None:
        details["device_fingerprint_sha256"] = hashlib.sha256(
            device_fingerprint.encode("utf-8")
        ).hexdigest()
    device_model = _require_non_empty_string(metadata, "device_model", errors)
    device_codename = _require_non_empty_string(metadata, "device_codename", errors)
    if device_model is not None:
        details["device_model"] = device_model
    if device_codename is not None:
        details["device_codename"] = device_codename
    _require_non_empty_string(metadata, "os_build_id", errors)
    minimum_os = _require_non_empty_string(metadata, "minimum_os", errors)
    _require_non_empty_string(metadata, "app_package_name", errors)
    _require_lowercase_sha256_hex(
        metadata,
        "app_signing_certificate_sha256",
        "slot.json",
        errors,
    )
    attestation_challenge_sha256 = _require_lowercase_sha256_hex(
        metadata,
        "attestation_challenge_sha256",
        "slot.json",
        errors,
    )
    details["attestation_challenge_sha256"] = attestation_challenge_sha256
    chain_digest = _require_lowercase_sha256_hex(
        metadata,
        "attestation_certificate_chain_sha256",
        "slot.json",
        errors,
    )
    chain_relative = _require_non_empty_string(
        metadata,
        "attestation_certificate_chain_path",
        errors,
    )
    if chain_relative is not None:
        chain_relative = _normalise_safe_relative_path(
            chain_relative,
            errors,
            "slot.json attestation_certificate_chain_path",
        )
    if chain_relative is not None:
        if not _safe_relative_path_is_child_of(chain_relative, "attestation"):
            errors.append(
                "slot.json attestation_certificate_chain_path must stay under attestation/"
            )
        else:
            chain_bytes, actual_chain_digest, digest_errors = (
                _metadata_artifact_bytes_and_sha256(
                    slot_path,
                    chain_relative,
                    "slot.json attestation_certificate_chain_path",
                    "slot.json attestation_certificate_chain_path must point to an existing file",
                )
            )
            if digest_errors:
                errors.extend(digest_errors)
            elif chain_bytes is not None and actual_chain_digest is not None:
                attestation_certificate_chain_bytes = chain_bytes
                _validate_attestation_certificate_chain_artifact(
                    chain_relative,
                    chain_bytes,
                    errors,
                )
                if chain_digest is not None:
                    if actual_chain_digest != chain_digest:
                        errors.append(
                            "slot.json attestation_certificate_chain_sha256 does not match attestation_certificate_chain_path"
                        )
                    else:
                        details["attestation_certificate_chain_path"] = chain_relative
                        details["attestation_certificate_chain_sha256"] = chain_digest
    _require_lowercase_sha256_hex(
        metadata,
        "offline_wallet_policy_sha256",
        "slot.json",
        errors,
    )
    apk_digest = _require_lowercase_sha256_hex(
        metadata,
        "offline_wallet_apk_sha256",
        "slot.json",
        errors,
    )
    apk_relative = _require_non_empty_string(metadata, "offline_wallet_apk_path", errors)
    if apk_relative is not None:
        apk_relative = _normalise_safe_relative_path(
            apk_relative,
            errors,
            "slot.json offline_wallet_apk_path",
        )
    if apk_relative is not None:
        if not _safe_relative_path_is_child_of(apk_relative, "evidence"):
            errors.append("slot.json offline_wallet_apk_path must stay under evidence/")
        else:
            _, actual_apk_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
                slot_path,
                apk_relative,
                "slot.json offline_wallet_apk_path",
                "slot.json offline_wallet_apk_path must point to an existing file",
                _slot_artifact_max_bytes(apk_relative),
            )
            if digest_errors:
                errors.extend(digest_errors)
            elif apk_digest is not None and actual_apk_digest is not None:
                if actual_apk_digest != apk_digest:
                    errors.append(
                        "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path"
                    )
                else:
                    details["offline_wallet_apk_path"] = apk_relative
                    details["offline_wallet_apk_sha256"] = apk_digest

    d2d_relative, d2d_digest, d2d_transport = validate_d2d_payment_transcript_binding(
        slot_path,
        metadata,
        errors,
    )
    if d2d_relative is not None and d2d_digest is not None:
        details["d2d_payment_transcript_path"] = d2d_relative
        details["d2d_payment_transcript_sha256"] = d2d_digest
        if d2d_transport is not None:
            details["d2d_payment_transport"] = d2d_transport
    d2d_transcripts = validate_d2d_payment_transcripts_binding(
        slot_path,
        metadata,
        errors,
        primary_relative=d2d_relative,
        primary_digest=d2d_digest,
        primary_transport=d2d_transport,
    )
    if (
        d2d_transcripts
        and metadata.get(D2D_PAYMENT_TRANSCRIPTS_FIELD) is not None
    ):
        details["d2d_payment_transcripts"] = d2d_transcripts
        details["d2d_payment_transports"] = sorted(d2d_transcripts)

    wallet_relative, wallet_digest = validate_wallet_integrity_transcript_binding(
        slot_path,
        metadata,
        errors,
    )
    if wallet_relative is not None and wallet_digest is not None:
        details["wallet_integrity_transcript_path"] = wallet_relative
        details["wallet_integrity_transcript_sha256"] = wallet_digest

    native_bridge_abi_version = _require_int(
        metadata,
        "native_bridge_abi_version",
        "slot.json",
        errors,
    )
    if (
        native_bridge_abi_version is not None
        and native_bridge_abi_version != REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION
    ):
        errors.append(
            "slot.json native_bridge_abi_version must be "
            f"{REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION}"
        )
    elif native_bridge_abi_version is not None:
        details["native_bridge_abi_version"] = native_bridge_abi_version

    _require_true(metadata, "strongbox_attestation", errors)
    _require_true(metadata, "physical_device_attestation", errors)
    _require_true(metadata, "one_use_key_rotation_passed", errors)
    _require_true(metadata, "rollback_rejection_passed", errors)
    _require_status(metadata, "abi6_recursive_spend_jni_probe", {"passed"}, errors)
    _require_status(
        metadata,
        "abi7_recursive_compact_jni_probe",
        ABI7_RECURSIVE_COMPACT_ONE_HOP_JNI_PROBE_STATES,
        errors,
    )
    _require_status(
        metadata,
        "abi7_recursive_compact_prover_state",
        ABI7_RECURSIVE_COMPACT_MULTI_HOP_PROVER_STATES,
        errors,
    )
    if family is not None and minimum_os is not None:
        expected_minimum_os = KAGEMUSHA_STANDARD_DEVICE_MINIMUM_OS.get(family)
        if expected_minimum_os is None:
            errors.append("slot.json device_family must be one of the standard Kagemusha families")
        elif minimum_os != expected_minimum_os:
            errors.append(
                f"slot.json minimum_os for {family} must be {expected_minimum_os}"
            )
    if family is not None and (
        device_model is not None or device_codename is not None
    ):
        inferred_family = infer_kagemusha_device_family(device_model, device_codename)
        if inferred_family is None:
            errors.append(
                "slot.json device_model/device_codename must identify a standard Kagemusha family"
            )
        elif inferred_family != family:
            errors.append("slot.json device_family must match device_model/device_codename")

    security_level = _require_non_empty_string(metadata, "keymint_security_level", errors)
    if security_level is not None and security_level not in STRONGBOX_LEVELS:
        errors.append("slot.json keymint_security_level must be STRONGBOX")
    attestation_result = validate_attestation_result(slot_path, metadata, errors)
    attestation_report = validate_attestation_report(slot_path, metadata, errors)
    validate_attestation_report_result_level_binding(
        attestation_result,
        attestation_report,
        errors,
    )
    validate_attestation_harness_result(
        slot_path,
        metadata,
        errors,
        attestation_certificate_chain_bytes=attestation_certificate_chain_bytes,
    )

    digest = _require_non_empty_string(metadata, "signed_evidence_artifact_sha256", errors)
    if digest is not None:
        if not SHA256_HEX_RE.fullmatch(digest):
            errors.append("slot.json signed_evidence_artifact_sha256 must be lowercase sha256 hex")
            digest = None
        elif digest == "0" * 64:
            errors.append(
                "slot.json signed_evidence_artifact_sha256 must be non-zero lowercase sha256 hex"
            )
            digest = None
    artifact_relative = _require_non_empty_string(
        metadata, "signed_evidence_artifact_path", errors
    )
    if artifact_relative is not None:
        if _path_has_surrounding_whitespace_component(Path(artifact_relative)):
            errors.append(
                "slot.json signed_evidence_artifact_path must not contain surrounding whitespace"
            )
            artifact_relative = None
        else:
            artifact_relative = _normalise_safe_relative_path(
                artifact_relative,
                errors,
                "slot.json signed_evidence_artifact_path",
            )
    artifact_root_ok = True
    if artifact_relative is not None and not _safe_relative_path_is_child_of(
        artifact_relative,
        "evidence",
    ):
        errors.append("slot.json signed_evidence_artifact_path must stay under evidence/")
        artifact_root_ok = False
    elif (
        artifact_relative is not None
        and artifact_relative != KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH
    ):
        errors.append(
            "slot.json signed_evidence_artifact_path must be "
            f"{KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH}"
        )
        artifact_root_ok = False
    if artifact_relative is not None and artifact_root_ok:
        artifact_path = slot_path / artifact_relative
        _, actual_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
            slot_path,
            artifact_relative,
            "slot.json signed_evidence_artifact_path",
            "slot.json signed_evidence_artifact_path must point to an existing file",
        )
        if digest_errors:
            errors.extend(digest_errors)
        elif (
            actual_digest is not None
            and digest is not None
            and actual_digest != digest
        ):
            errors.append(
                "slot.json signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path"
            )
        elif (
            actual_digest is not None
            and digest is not None
        ):
            details["signed_evidence_artifact_sha256"] = digest
            signed_evidence_details = validate_signed_evidence_artifact(
                slot_path,
                artifact_path,
                metadata,
                trusted_signer_public_keys,
                errors,
            )
            signed_at_utc = signed_evidence_details.get("signed_at_utc")
            if isinstance(signed_at_utc, str):
                details["signed_at_utc"] = signed_at_utc
            signer_public_key_sha256 = signed_evidence_details.get(
                "signer_public_key_sha256"
            )
            if signer_public_key_sha256 is not None:
                details["signed_evidence_signer_public_key_sha256"] = (
                    signer_public_key_sha256
                )

    commands = metadata.get("raw_test_commands")
    if not isinstance(commands, list) or not commands:
        errors.append("slot.json raw_test_commands must be a non-empty array")
    else:
        for index, command in enumerate(commands):
            if not isinstance(command, str) or not command.strip():
                errors.append(f"slot.json raw_test_commands[{index}] must be a non-empty string")
                continue
            if command != command.strip():
                errors.append(
                    f"slot.json raw_test_commands[{index}] must not contain surrounding whitespace"
                )
                continue
            if _contains_control_character(command):
                errors.append(
                    f"slot.json raw_test_commands[{index}] must not contain control characters"
                )
                continue
            if SECRET_RE.search(command):
                errors.append(
                    f"slot.json raw_test_commands[{index}] must not contain secret-looking material"
                )
                continue
        _validate_raw_test_command_markers(
            commands,
            label="slot.json raw_test_commands",
            errors=errors,
        )

    return errors, details


def scan_slot(
    slot_path: Path,
    require_kagemusha_production_evidence: bool = False,
    trusted_signer_public_keys: dict[str, Path] | None = None,
) -> dict:
    """Inspect a single slot directory and report any missing artefacts."""
    errors: list[str] = []
    present: dict[str, bool] = {}
    file_counts: dict[str, int] = {}
    slot_label = _display_slot_name(slot_path.name)

    if SECRET_RE.search(slot_path.name):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory name must not contain secret-looking material"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }
    if any(character.isspace() for character in slot_path.name):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory name must not contain whitespace"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }
    if _contains_control_character(slot_path.name):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory name must not contain control characters"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }
    if "\\" in slot_path.name:
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory name must not contain backslashes"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    try:
        slot_mode = slot_path.lstat().st_mode
    except FileNotFoundError:
        slot_mode = None
    except OSError:
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory metadata could not be read"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    if slot_mode is not None and stat.S_ISLNK(slot_mode):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory must not be a symlink"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    try:
        parent_mode = slot_path.parent.lstat().st_mode
    except FileNotFoundError:
        parent_mode = None
    except OSError:
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot parent directory metadata could not be read"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    if parent_mode is not None and stat.S_ISLNK(parent_mode):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot parent directory must not be a symlink"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    ancestor_errors = validate_no_symlink_ancestors(
        slot_path,
        "slot ancestor directory",
    )
    if ancestor_errors:
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ancestor_errors,
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    if slot_mode is None or not stat.S_ISDIR(slot_mode):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory missing"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    validate_no_slot_symlink_artifacts(slot_path, errors)
    validate_slot_regular_file_artifacts(slot_path, errors)
    validate_no_slot_hardlink_artifacts(slot_path, errors)

    for dirname in EXPECTED_DIRS:
        dir_path = slot_path / dirname
        directory_present, directory_missing = _slot_expected_directory_present(
            dir_path,
            dirname,
            errors,
        )
        present[dirname] = directory_present
        if directory_missing:
            errors.append(f"missing {dirname}/ directory")
        if not directory_present:
            continue
        entries = _slot_tree_entries(dir_path, f"{dirname}/", errors)
        if entries is None:
            continue
        count = _slot_regular_file_count(slot_path, entries, errors)
        file_counts[dirname] = count
        if count == 0:
            errors.append(f"{dirname}/ contains no files")

    sha_path = slot_path / "sha256sum.txt"
    present["sha256sum.txt"] = _slot_regular_file_present(
        sha_path,
        "sha256sum.txt",
        errors,
    )
    errors.extend(verify_sha256_manifest(slot_path))

    kagemusha: dict[str, Any] = {"required": require_kagemusha_production_evidence}
    if require_kagemusha_production_evidence:
        metadata_errors, metadata_details = validate_kagemusha_production_metadata(
            slot_path,
            trusted_signer_public_keys,
        )
        errors.extend(metadata_errors)
        kagemusha.update(metadata_details)

    status = "ok" if not errors else "error"
    return {
        "slot": slot_label,
        "status": status,
        "errors": errors,
        "present": present,
        "file_counts": file_counts,
        "kagemusha": kagemusha,
    }


def _slot_expected_directory_present(
    dir_path: Path,
    dirname: str,
    errors: list[str],
) -> tuple[bool, bool]:
    """Return whether an expected slot directory is usable and whether it is missing."""

    try:
        dir_mode = dir_path.lstat().st_mode
    except FileNotFoundError:
        return False, True
    except OSError:
        _append_error_once(errors, f"{dirname}/ metadata could not be read")
        return False, False
    if stat.S_ISLNK(dir_mode) or not stat.S_ISDIR(dir_mode):
        return False, False
    return True, False


def _slot_regular_file_count(
    slot_path: Path,
    entries: list[Path],
    errors: list[str],
) -> int:
    """Count regular slot artifact files without following aliases."""

    count = 0
    for entry in entries:
        relative = entry.relative_to(slot_path).as_posix()
        try:
            entry_mode = entry.lstat().st_mode
        except OSError:
            _append_error_once(
                errors,
                f"slot artifact {_display_path(relative)} file metadata could not be read",
            )
            continue
        if stat.S_ISREG(entry_mode):
            count += 1
    return count


def _slot_regular_file_present(path: Path, label: str, errors: list[str]) -> bool:
    """Return whether a slot artifact leaf is a regular file without following aliases."""

    try:
        mode = path.lstat().st_mode
    except FileNotFoundError:
        return False
    except OSError:
        _append_error_once(errors, f"{label} file metadata could not be read")
        return False
    return stat.S_ISREG(mode)


def discover_slots(
    root: Path, slot_ids: Iterable[str] | None
) -> tuple[list[Path], list[str]]:
    """List slot directories under the given root."""
    if slot_ids is not None:
        validated_slot_ids, slot_id_errors = validate_slot_ids(slot_ids)
        if slot_id_errors:
            return [], slot_id_errors
        assert validated_slot_ids is not None
        return [root / slot for slot in validated_slot_ids], []
    try:
        entries = sorted(root.iterdir(), key=lambda entry: entry.name)
    except OSError:
        return [], ["device-lab root could not be listed"]
    slots: list[Path] = []
    errors: list[str] = []
    for entry in entries:
        try:
            entry_mode = entry.lstat().st_mode
        except OSError:
            _append_error_once(
                errors,
                "device-lab slot directory metadata could not be read",
            )
            continue
        if stat.S_ISDIR(entry_mode) or stat.S_ISLNK(entry_mode):
            slots.append(entry)
    return slots, errors


def build_summary(
    root: Path,
    reports: list[dict],
    *,
    require_kagemusha_production_evidence: bool = False,
    require_kagemusha_standard_matrix: bool = False,
    trusted_signer_public_keys: dict[str, Path] | None = None,
) -> dict:
    now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
    summary_reports = [_summary_safe_report(report) for report in reports]
    require_complete_kagemusha = (
        require_kagemusha_production_evidence
        or require_kagemusha_standard_matrix
    )
    trusted_signer_public_key_sha256 = _trusted_signer_public_key_sha256_set(
        trusted_signer_public_keys
    )
    output_reports = _summary_reports_for_release_output(
        summary_reports,
        require_complete_signed_evidence=require_complete_kagemusha,
        trusted_signer_public_key_sha256=trusted_signer_public_key_sha256,
    )
    summary = {
        "schema_version": 1,
        "generated_at": now.isoformat().replace("+00:00", "Z"),
        "root": DEVICE_LAB_ROOT_SUMMARY_LABEL,
        "slots": output_reports,
        "ok": sum(1 for r in output_reports if r["status"] == "ok"),
        "failed": sum(1 for r in output_reports if r["status"] != "ok"),
    }
    if require_kagemusha_production_evidence or require_kagemusha_standard_matrix:
        covered = sorted(
            {
                family
                for report in summary_reports
                for family in [
                    (
                        _summary_release_device_family(
                            report,
                            trusted_signer_public_key_sha256,
                        )
                        if require_complete_kagemusha
                        else _summary_device_family(report)
                    )
                ]
                if family is not None
            }
        )
        missing = [
            family
            for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            if family not in covered
        ]
        covered_d2d_payment_transports = sorted(
            {
                transport
                for report in summary_reports
                for transport in _summary_release_d2d_payment_transports(
                    report,
                    trusted_signer_public_key_sha256,
                )
            }
        )
        missing_d2d_payment_transports = [
            transport
            for transport in sorted(D2D_PAYMENT_TRANSPORTS)
            if transport not in covered_d2d_payment_transports
        ]
        summary["kagemusha"] = {
            "production_evidence_required": require_kagemusha_production_evidence,
            "standard_matrix_required": require_kagemusha_standard_matrix,
            "required_device_families": list(KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "covered_device_families": covered,
            "missing_device_families": missing,
            "required_d2d_payment_transports": sorted(D2D_PAYMENT_TRANSPORTS),
            "covered_d2d_payment_transports": covered_d2d_payment_transports,
            "missing_d2d_payment_transports": missing_d2d_payment_transports,
            "duplicate_bindings": kagemusha_duplicate_matrix_bindings(
                summary_reports,
                require_complete_signed_evidence=require_complete_kagemusha,
                trusted_signer_public_key_sha256=(
                    trusted_signer_public_key_sha256
                    if require_complete_kagemusha
                    else None
                ),
            ),
            "trusted_signer_public_key_sha256": sorted(
                trusted_signer_public_key_sha256
            ),
        }
    return summary


def kagemusha_duplicate_matrix_bindings(
    reports: list[dict],
    *,
    require_complete_signed_evidence: bool = False,
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> dict[str, list[dict[str, Any]]]:
    """Return duplicated physical-device bindings without exposing raw values."""

    duplicates: dict[str, list[dict[str, Any]]] = {}
    for field in ("device_fingerprint_sha256", "attestation_challenge_sha256"):
        seen: dict[str, list[str]] = {}
        for report in reports:
            if report.get("status") != "ok":
                continue
            slot = report.get("slot")
            kagemusha = (
                _summary_release_kagemusha(
                    report,
                    trusted_signer_public_key_sha256,
                )
                if require_complete_signed_evidence
                else _summary_kagemusha(report)
            )
            if kagemusha is None:
                continue
            value = kagemusha.get(field)
            if (
                not isinstance(slot, str)
                or not isinstance(value, str)
                or not SHA256_HEX_RE.fullmatch(value)
                or value == "0" * 64
            ):
                continue
            seen.setdefault(value, []).append(_display_slot_name(slot))
        for value, slots in sorted(seen.items()):
            if len(slots) <= 1:
                continue
            duplicates.setdefault(field, []).append(
                {
                    "slots": sorted(slots),
                    "value_sha256": value,
                }
            )
    return duplicates


def validate_summary_output_path(path: Path, label: str) -> list[str]:
    """Reject summary output paths that could overwrite aliased local files."""

    path_text = str(path)
    if SECRET_RE.search(path_text):
        return [f"{label} must not contain secret-looking material"]
    if _contains_control_character(path_text):
        return [f"{label} must not contain control characters"]
    if path_text != path_text.strip() or _path_has_surrounding_whitespace_component(
        path
    ):
        return [f"{label} must not contain surrounding whitespace"]
    if "\\" in path_text:
        return [f"{label} must not contain backslashes"]
    if ".." in path.parts:
        return [f"{label} must be canonical"]
    parent = path.parent
    parent_exists, parent_errors = _validate_summary_output_parent(path, label)
    if parent_errors:
        return parent_errors
    ancestor_errors = validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    if not parent_exists:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    parent_exists, parent_errors = _validate_summary_output_parent(
        path,
        label,
        missing_error=f"{label} parent must be a directory",
    )
    if parent_errors:
        return parent_errors
    if not parent_exists:
        return [f"{label} parent must be a directory"]
    ancestor_errors = validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    try:
        output_mode = path.lstat().st_mode
    except FileNotFoundError:
        return []
    except OSError:
        return [f"{label} file metadata could not be read"]
    if stat.S_ISLNK(output_mode):
        return [f"{label} must not be a symlink"]
    if not stat.S_ISREG(output_mode):
        return [f"{label} must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return [f"{label} hardlink metadata could not be read"]
    if link_count > 1:
        return [f"{label} must not be hardlinked"]
    return []


def _validate_summary_output_parent(
    path: Path,
    label: str,
    *,
    missing_error: str | None = None,
) -> tuple[bool, list[str]]:
    """Classify a scanner summary output parent without following aliases."""

    parent = path.parent
    try:
        parent_mode = parent.lstat().st_mode
    except FileNotFoundError:
        if missing_error is None:
            return False, []
        return False, [missing_error]
    except OSError:
        return False, [f"{label} parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_mode):
        return True, [f"{label} parent directory must not be a symlink"]
    if not stat.S_ISDIR(parent_mode):
        return True, [f"{label} parent must be a directory"]
    return True, []


def _file_identity(file_stat: os.stat_result) -> tuple[int, int]:
    return file_stat.st_dev, file_stat.st_ino


def _directory_open_flags() -> int:
    flags = os.O_RDONLY
    if hasattr(os, "O_DIRECTORY"):
        flags |= os.O_DIRECTORY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    return flags


def _sync_summary_output_parent(
    parent: Path,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        parent_fd = os.open(parent, _directory_open_flags())
    except OSError:
        return [f"{label} parent directory could not be synced"]
    try:
        return _sync_summary_output_parent_fd(
            parent_fd,
            label,
            expected_identity=expected_identity,
        )
    finally:
        os.close(parent_fd)


def _sync_summary_output_parent_fd(
    parent_fd: int,
    label: str,
    *,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    try:
        parent_stat = os.fstat(parent_fd)
        if not stat.S_ISDIR(parent_stat.st_mode):
            return [f"{label} parent directory could not be synced"]
        if expected_identity is not None and _file_identity(parent_stat) != expected_identity:
            return [f"{label} parent directory changed before sync"]
        os.fsync(parent_fd)
    except OSError:
        return [f"{label} parent directory could not be synced"]
    return []


def _read_summary_output_text(
    path: Path,
    expected_stat: os.stat_result,
) -> tuple[str | None, list[str]]:
    """Read scanner summary output text without trusting a stale path."""

    chunks: list[bytes] = []
    summary_expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    try:
        with path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = path.lstat()
            if stat.S_ISLNK(path_stat.st_mode):
                return None, ["--json-out must not be a symlink"]
            if not stat.S_ISREG(path_stat.st_mode) or not stat.S_ISREG(
                open_stat.st_mode
            ):
                return None, ["--json-out must be a regular file"]
            summary_open_identity = (open_stat.st_dev, open_stat.st_ino)
            if summary_open_identity != summary_expected_identity or (
                path_stat.st_dev,
                path_stat.st_ino,
            ) != summary_expected_identity:
                return None, ["--json-out changed while being read"]
            if open_stat.st_nlink > 1:
                return None, ["--json-out must not be hardlinked"]
            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                return None, [
                    "--json-out must be no more than "
                    f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                ]
            size = 0
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                size += len(chunk)
                if size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
                    return None, [
                        "--json-out must be no more than "
                        f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
                    ]
                chunks.append(chunk)
            final_path_stat = path.lstat()
            if (
                final_path_stat.st_dev,
                final_path_stat.st_ino,
            ) != summary_expected_identity:
                return None, ["--json-out changed while being read"]
    except OSError:
        return None, ["--json-out write verification failed"]
    try:
        return b"".join(chunks).decode("utf-8"), []
    except UnicodeDecodeError:
        return None, ["--json-out write verification failed"]


def write_summary(path: Path, summary: dict) -> list[str]:
    errors = validate_summary_output_path(path, "--json-out")
    if errors:
        return errors
    try:
        parent_stat = path.parent.lstat()
    except OSError:
        return ["--json-out parent directory metadata could not be read"]
    if stat.S_ISLNK(parent_stat.st_mode) or not stat.S_ISDIR(parent_stat.st_mode):
        return ["--json-out parent directory could not be synced"]
    parent_identity = _file_identity(parent_stat)
    try:
        summary_text = json.dumps(summary, indent=2, allow_nan=False) + "\n"
    except ValueError:
        return ["--json-out summary is not strict JSON"]
    if len(summary_text.encode("utf-8")) > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return [
            "--json-out must be no more than "
            f"{MAX_ANDROID_DEVICE_LAB_JSON_BYTES} bytes"
        ]
    tmp_path: Path | None = None
    tmp_identity: tuple[int, int] | None = None
    parent_fd: int | None = None
    write_errors: list[str] = []
    try:
        try:
            parent_fd = os.open(path.parent, _directory_open_flags())
        except OSError:
            return ["--json-out parent directory could not be synced"]
        try:
            parent_fd_stat = os.fstat(parent_fd)
        except OSError:
            return ["--json-out parent directory could not be synced"]
        if (
            not stat.S_ISDIR(parent_fd_stat.st_mode)
            or _file_identity(parent_fd_stat) != parent_identity
        ):
            return ["--json-out parent directory changed before sync"]
        with tempfile.NamedTemporaryFile(
            "w",
            dir=path.parent,
            encoding="utf-8",
            prefix=f".{path.name}.",
            suffix=".tmp",
            delete=False,
        ) as handle:
            tmp_path = Path(handle.name)
            tmp_identity = _file_identity(os.fstat(handle.fileno()))
            handle.write(summary_text)
            handle.flush()
            os.fsync(handle.fileno())
        errors = validate_summary_output_path(path, "--json-out")
        if errors:
            write_errors.extend(errors)
        else:
            os.replace(
                tmp_path.name,
                path.name,
                src_dir_fd=parent_fd,
                dst_dir_fd=parent_fd,
            )
            tmp_path = None
            try:
                installed_stat = os.stat(
                    path.name,
                    dir_fd=parent_fd,
                    follow_symlinks=False,
                )
            except OSError:
                write_errors.append("--json-out write verification failed")
            else:
                installed_identity = _file_identity(installed_stat)
                if stat.S_ISLNK(installed_stat.st_mode):
                    write_errors.append("--json-out must not be a symlink")
                elif not stat.S_ISREG(installed_stat.st_mode):
                    write_errors.append("--json-out must be a regular file")
                else:
                    try:
                        current_parent_stat = path.parent.lstat()
                    except OSError:
                        cleanup_errors = _unlink_summary_output_if_identity_at(
                            parent_fd,
                            path.name,
                            installed_identity,
                        )
                        write_errors.extend(
                            [
                                "--json-out parent directory metadata could not be read",
                                *cleanup_errors,
                            ]
                        )
                    else:
                        if _file_identity(current_parent_stat) != parent_identity:
                            cleanup_errors = _unlink_summary_output_if_identity_at(
                                parent_fd,
                                path.name,
                                installed_identity,
                            )
                            write_errors.extend(
                                [
                                    "--json-out parent directory changed before sync",
                                    *cleanup_errors,
                                ]
                            )
                        else:
                            sync_errors = _sync_summary_output_parent_fd(
                                parent_fd,
                                "--json-out",
                                expected_identity=parent_identity,
                            )
                            if sync_errors:
                                cleanup_errors = _unlink_summary_output_if_identity_at(
                                    parent_fd,
                                    path.name,
                                    installed_identity,
                                )
                                write_errors.extend([*sync_errors, *cleanup_errors])
    except OSError:
        write_errors.append("--json-out could not be written")
    finally:
        if tmp_path is not None:
            write_errors.extend(_cleanup_summary_output(tmp_path, tmp_identity))
        if parent_fd is not None:
            os.close(parent_fd)
    if write_errors:
        return write_errors
    errors = validate_summary_output_path(path, "--json-out")
    if errors:
        return errors
    try:
        expected_stat = path.lstat()
    except (FileNotFoundError, OSError):
        return ["--json-out write verification failed"]
    if stat.S_ISLNK(expected_stat.st_mode):
        return ["--json-out must not be a symlink"]
    if not stat.S_ISREG(expected_stat.st_mode):
        return ["--json-out must be a regular file"]
    try:
        link_count = path.stat().st_nlink
    except OSError:
        return ["--json-out hardlink metadata could not be read"]
    if link_count > 1:
        return ["--json-out must not be hardlinked"]
    readback_text, readback_errors = _read_summary_output_text(path, expected_stat)
    if readback_errors:
        return readback_errors
    if readback_text != summary_text:
        return ["--json-out write verification failed"]
    return []


def _unlink_summary_output_if_identity_at(
    parent_fd: int,
    name: str,
    expected_identity: tuple[int, int],
) -> list[str]:
    try:
        output_stat = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
    except FileNotFoundError:
        return []
    except OSError:
        return ["--json-out rollback cleanup metadata could not be read"]
    if not stat.S_ISREG(output_stat.st_mode) or _file_identity(output_stat) != expected_identity:
        return []
    try:
        os.unlink(name, dir_fd=parent_fd)
    except FileNotFoundError:
        return []
    except OSError:
        return ["--json-out could not be removed after parent sync failure"]
    try:
        os.fsync(parent_fd)
    except OSError:
        return ["--json-out cleanup could not be synced after parent sync failure"]
    return []


def _cleanup_summary_output(
    path: Path,
    expected_identity: tuple[int, int] | None,
) -> list[str]:
    if expected_identity is None:
        return ["--json-out temporary file metadata could not be read"]
    try:
        parent_fd = os.open(path.parent, _directory_open_flags())
    except OSError:
        return ["--json-out temporary file could not be removed"]
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
            return ["--json-out temporary file could not be removed"]
        if (
            not stat.S_ISREG(temp_stat.st_mode)
            or _file_identity(temp_stat) != expected_identity
        ):
            return ["--json-out temporary file changed before cleanup"]
        try:
            os.unlink(path.name, dir_fd=parent_fd)
        except FileNotFoundError:
            return []
        except OSError:
            return ["--json-out temporary file could not be removed"]
        try:
            os.fsync(parent_fd)
        except OSError:
            return ["--json-out temporary file cleanup could not be synced"]
    finally:
        os.close(parent_fd)
    return []


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Validate Android device-lab slots for AND6 compliance."
    )
    parser.add_argument(
        "--root",
        default="artifacts/android/device_lab",
        help="Root directory containing device-lab slots.",
    )
    parser.add_argument(
        "--slot",
        action="append",
        dest="slots",
        default=None,
        help="Specific slot id(s) to validate. Defaults to all slots under --root.",
    )
    parser.add_argument(
        "--require-slot",
        action="store_true",
        help="Fail if no slot directories are found.",
    )
    parser.add_argument(
        "--json-out",
        default=None,
        help="Optional path to write a JSON summary.",
    )
    parser.add_argument(
        "--allow-missing-root",
        action="store_true",
        help="Treat a missing root directory as a skip instead of an error.",
    )
    parser.add_argument(
        "--require-kagemusha-production-evidence",
        action="store_true",
        help="Require each slot to carry Kagemusha production evidence metadata.",
    )
    parser.add_argument(
        "--require-kagemusha-standard-matrix",
        action="store_true",
        help=(
            "Require production evidence for every standard Kagemusha device family "
            "and offline D2D payment transport."
        ),
    )
    parser.add_argument(
        "--trusted-signer-public-key",
        action="append",
        dest="trusted_signer_public_keys",
        default=None,
        help="PEM public key for a trusted Android lab evidence signer.",
    )
    args = parser.parse_args(argv)

    path_arg_errors = []
    if SECRET_RE.search(args.root):
        path_arg_errors.append("--root must not contain secret-looking material")
    if _contains_control_character(args.root):
        path_arg_errors.append("--root must not contain control characters")
    path_arg_errors.extend(_cli_path_alias_errors(args.root, "--root"))
    if args.json_out is not None and SECRET_RE.search(args.json_out):
        path_arg_errors.append("--json-out must not contain secret-looking material")
    if args.json_out is not None and _contains_control_character(args.json_out):
        path_arg_errors.append("--json-out must not contain control characters")
    if args.json_out is not None:
        path_arg_errors.extend(_cli_path_alias_errors(args.json_out, "--json-out"))
    for index, key_path in enumerate(args.trusted_signer_public_keys or []):
        label = f"--trusted-signer-public-key[{index}]"
        if SECRET_RE.search(key_path):
            path_arg_errors.append(f"{label} must not contain secret-looking material")
        if _contains_control_character(key_path):
            path_arg_errors.append(f"{label} must not contain control characters")
        path_arg_errors.extend(_cli_path_alias_errors(key_path, label))
    if path_arg_errors:
        for error in path_arg_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1

    slot_ids, slot_id_errors = validate_slot_ids(args.slots)
    if slot_id_errors:
        for error in slot_id_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1

    root = Path(args.root)
    root_exists, root_errors = classify_device_lab_root_path(root)
    if root_errors:
        for error in root_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1
    if not root_exists:
        if args.allow_missing_root:
            print("[device-lab] root missing; skipping")
            return 0
        print("[device-lab] root does not exist", file=sys.stderr)
        return 1

    slot_paths, discovery_errors = discover_slots(root, slot_ids)
    if discovery_errors:
        for error in discovery_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1
    if not slot_paths:
        if args.require_slot:
            print("[device-lab] no slots found under root", file=sys.stderr)
            return 1
        print("[device-lab] no slots found under root; nothing to check")
        return 0

    reports: list[dict] = []
    failures = 0
    require_kagemusha = (
        args.require_kagemusha_production_evidence
        or args.require_kagemusha_standard_matrix
    )
    trusted_signer_public_keys, signer_errors = load_trusted_signer_public_keys(
        args.trusted_signer_public_keys
    )
    if require_kagemusha and signer_errors:
        for error in signer_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1
    for slot_path in slot_paths:
        report = scan_slot(
            slot_path,
            require_kagemusha_production_evidence=require_kagemusha,
            trusted_signer_public_keys=trusted_signer_public_keys,
        )
        reports.append(report)
        slot_display = report.get("slot", _display_path(slot_path.name))
        if report["status"] != "ok":
            failures += 1
            print(f"[device-lab] {slot_display}: {', '.join(report['errors'])}", file=sys.stderr)
        else:
            print(f"[device-lab] {slot_display}: ok")

    if args.require_kagemusha_standard_matrix:
        trusted_signer_public_key_sha256 = _trusted_signer_public_key_sha256_set(
            trusted_signer_public_keys
        )
        covered = {
            family
            for report in reports
            for family in [
                _summary_release_device_family(
                    report,
                    trusted_signer_public_key_sha256,
                )
            ]
            if family is not None
        }
        missing = [
            family
            for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            if family not in covered
        ]
        covered_d2d_payment_transports = {
            transport
            for report in reports
            for transport in _summary_release_d2d_payment_transports(
                report,
                trusted_signer_public_key_sha256,
            )
        }
        missing_d2d_payment_transports = [
            transport
            for transport in sorted(D2D_PAYMENT_TRANSPORTS)
            if transport not in covered_d2d_payment_transports
        ]
        if missing:
            failures += 1
            print(
                "[device-lab] missing Kagemusha production evidence for device families: "
                + ", ".join(missing),
                file=sys.stderr,
            )
        if missing_d2d_payment_transports:
            failures += 1
            print(
                "[device-lab] missing Kagemusha production evidence for D2D "
                "payment transports: "
                + ", ".join(missing_d2d_payment_transports),
                file=sys.stderr,
            )
    if require_kagemusha:
        duplicate_bindings = kagemusha_duplicate_matrix_bindings(
            reports,
            require_complete_signed_evidence=True,
            trusted_signer_public_key_sha256=_trusted_signer_public_key_sha256_set(
                trusted_signer_public_keys
            ),
        )
        for field, entries in sorted(duplicate_bindings.items()):
            for entry in entries:
                failures += 1
                print(
                    "[device-lab] duplicate Kagemusha "
                    f"{field} across slots: {', '.join(entry['slots'])}",
                    file=sys.stderr,
                )

    summary = build_summary(
        root,
        reports,
        require_kagemusha_production_evidence=require_kagemusha,
        require_kagemusha_standard_matrix=args.require_kagemusha_standard_matrix,
        trusted_signer_public_keys=trusted_signer_public_keys,
    )
    if args.json_out:
        write_errors = write_summary(Path(args.json_out), summary)
        if write_errors:
            for error in write_errors:
                print(f"[device-lab] {error}", file=sys.stderr)
            return 1
        print("[device-lab] wrote summary")

    return 1 if failures else 0


if __name__ == "__main__":
    import sys

    sys.exit(main())
