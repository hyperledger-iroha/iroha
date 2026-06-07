"""Validate Android device-lab slots for AND6 compliance evidence."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
from pathlib import Path
from pathlib import PurePosixPath
import re
import shutil
import stat
import subprocess
import sys
import tempfile
from typing import Any, Iterable, List


EXPECTED_DIRS: tuple[str, ...] = ("telemetry", "attestation", "queue", "logs")
OPTIONAL_EVIDENCE_DIRS: tuple[str, ...] = ("evidence", "handoff", "wallet")
REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS: tuple[str, ...] = (
    "telemetry/telemetry.json",
    "telemetry/status.ndjson",
    "attestation/result.json",
    "queue/pending_queue.json",
    "logs/runtime.log",
)
KAGEMUSHA_SIGNED_EVIDENCE_ARTIFACT_PATH = "evidence/signed-evidence.json"
MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 16 * 1024 * 1024
KAGEMUSHA_RUNTIME_LOG_COMPLETE_MARKER = "kagemusha device-lab run complete"
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
DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
SHA256_HEX_RE = re.compile(r"^[0-9a-f]{64}$")
SIGNED_AT_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
SECRET_RE = re.compile(
    r"(authorization:|bearer\s+|private[_-]?key|token=|x-iroha-signature)",
    re.IGNORECASE,
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
RAW_TEST_COMMAND_REQUIRED_MARKERS: tuple[str, ...] = (
    ":client-android:assembleRelease",
    ":offline-wallet-android:assembleRelease",
    "connectedAndroidTest",
    "KagemushaRecursiveSpendProverTest",
    "OfflineNoteTransferHandoff",
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMAND = (
    "ANDROID_HARNESS_MAINS="
    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest "
    "./gradlew :client-android:assembleRelease "
    ":offline-wallet-android:assembleRelease connectedAndroidTest "
    "-Pandroid.testInstrumentationRunnerArguments.class="
    "org.hyperledger.iroha.android.offline.KagemushaRecursiveSpendProverTest,"
    "org.hyperledger.iroha.android.offline.OfflineNoteTransferHandoffTest"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS: tuple[str, ...] = (
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMAND,
)
SIGNED_EVIDENCE_SCHEMA = "iroha.android.device_lab.kagemusha.signed_evidence.v1"
D2D_PAYMENT_TRANSCRIPT_SCHEMA = "iroha.android.device_lab.kagemusha.d2d_payment.v1"
D2D_PAYMENT_PAYLOAD_SCHEMA = "kagemusha.recursive_spend.reserved_lineage.d2d.v1"
WALLET_INTEGRITY_TRANSCRIPT_SCHEMA = (
    "iroha.android.device_lab.kagemusha.wallet_integrity.v1"
)
D2D_PAYMENT_TRANSPORTS = {"nearby_offline", "nfc_hce", "qr"}
MAX_D2D_PAYMENT_PAYLOAD_BYTES = 16 * 1024
ATTESTATION_CERTIFICATE_CHAIN_SUFFIXES = (".der", ".pem")
MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES = 64 * 1024
SIGNED_EVIDENCE_SIGNATURE_ALGORITHMS = {"ed25519"}
REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION = 7
SIGNED_EVIDENCE_SLOT_STRING_FIELDS: tuple[str, ...] = (
    "slot_id",
    "device_family",
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
SLOT_METADATA_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *SIGNED_EVIDENCE_SLOT_STRING_FIELDS,
        *SIGNED_EVIDENCE_SLOT_SHA256_FIELDS,
        *SIGNED_EVIDENCE_SLOT_INT_FIELDS,
        *SIGNED_EVIDENCE_SLOT_TRUE_FIELDS,
        "raw_test_commands",
        "signed_evidence_artifact_path",
        "signed_evidence_artifact_sha256",
    }
)
SIGNED_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        "slot_id",
        "device_family",
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


def _slot_files(slot_path: Path) -> set[str]:
    if slot_path.is_symlink() or not slot_path.is_dir():
        return set()
    if SECRET_RE.search(str(slot_path)):
        return set()
    if validate_no_symlink_ancestors(slot_path, "slot ancestor directory"):
        return set()
    files: set[str] = set()
    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        if dir_path.is_symlink() or not dir_path.is_dir():
            continue
        for entry in dir_path.rglob("*"):
            if entry.is_file() or entry.is_symlink():
                files.add(entry.relative_to(slot_path).as_posix())
    skipped_roots = {"sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    for entry in slot_path.iterdir():
        if entry.name in skipped_roots:
            continue
        if entry.is_file() or entry.is_symlink():
            files.add(entry.relative_to(slot_path).as_posix())
    return files


def _slot_relative_symlink_ancestor(slot_path: Path, relative: str) -> str | None:
    current = slot_path
    for part in PurePosixPath(relative).parts[:-1]:
        current = current / part
        try:
            if current.is_symlink():
                return current.relative_to(slot_path).as_posix()
        except OSError:
            return current.relative_to(slot_path).as_posix()
    return None


def validate_no_slot_symlink_artifacts(slot_path: Path, errors: list[str]) -> None:
    """Reject symlinked slot metadata, directories, and evidence artifacts."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in ("slot.json", "sha256sum.txt"):
        if (slot_path / relative).is_symlink():
            errors.append(f"{relative} must not be a symlink")

    for dirname in EXPECTED_DIRS + OPTIONAL_EVIDENCE_DIRS:
        dir_path = slot_path / dirname
        if dir_path.is_symlink():
            errors.append(f"{dirname}/ must not be a symlink")
            continue
        if not dir_path.exists():
            continue
        for entry in dir_path.rglob("*"):
            if entry.is_symlink():
                relative = entry.relative_to(slot_path).as_posix()
                errors.append(
                    f"slot artifact {_display_path(relative)} must not be a symlink"
                )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    for entry in slot_path.iterdir():
        if entry.name in skipped_roots:
            continue
        if entry.is_symlink():
            relative = entry.relative_to(slot_path).as_posix()
            errors.append(f"slot artifact {_display_path(relative)} must not be a symlink")


def _reject_hardlinked_file(path: Path, label: str, errors: list[str]) -> None:
    if path.is_symlink() or not path.is_file():
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
        if dir_path.is_symlink() or not dir_path.exists():
            continue
        for entry in dir_path.rglob("*"):
            if entry.is_file() and not entry.is_symlink():
                relative = entry.relative_to(slot_path).as_posix()
                _reject_hardlinked_file(
                    entry,
                    f"slot artifact {_display_path(relative)}",
                    errors,
                )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    for entry in slot_path.iterdir():
        if entry.name in skipped_roots:
            continue
        if entry.is_file() and not entry.is_symlink():
            relative = entry.relative_to(slot_path).as_posix()
            _reject_hardlinked_file(
                entry,
                f"slot artifact {_display_path(relative)}",
                errors,
            )


def _reject_non_regular_file(path: Path, label: str, errors: list[str]) -> None:
    if path.is_symlink() or not path.exists():
        return
    try:
        mode = path.lstat().st_mode
    except OSError:
        errors.append(f"{label} file metadata could not be read")
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
        if dir_path.is_symlink() or not dir_path.exists():
            continue
        try:
            mode = dir_path.lstat().st_mode
        except OSError:
            errors.append(f"{dirname}/ metadata could not be read")
            continue
        if not stat.S_ISDIR(mode):
            errors.append(f"{dirname}/ must be a directory")
            continue
        for entry in dir_path.rglob("*"):
            if entry.is_symlink():
                continue
            try:
                entry_mode = entry.lstat().st_mode
            except OSError:
                relative = entry.relative_to(slot_path).as_posix()
                errors.append(
                    f"slot artifact {_display_path(relative)} file metadata could not be read"
                )
                continue
            if stat.S_ISDIR(entry_mode):
                continue
            if not stat.S_ISREG(entry_mode):
                relative = entry.relative_to(slot_path).as_posix()
                errors.append(
                    f"slot artifact {_display_path(relative)} must be a regular file"
                )

    skipped_roots = {"slot.json", "sha256sum.txt", *EXPECTED_DIRS, *OPTIONAL_EVIDENCE_DIRS}
    for entry in slot_path.iterdir():
        if entry.name in skipped_roots or entry.is_symlink():
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
    path_text = path_text.strip()
    if path_text.startswith("*"):
        path_text = path_text[1:]
    if SECRET_RE.search(path_text):
        errors.append(f"{label}: unsafe path contains secret-looking material")
        return None
    candidate = PurePosixPath(path_text)
    if (
        not path_text
        or path_text.startswith("/")
        or "\\" in path_text
        or candidate.is_absolute()
        or ".." in candidate.parts
        or candidate.as_posix() in {"", "."}
        or (not allow_sha_manifest and candidate.as_posix() == "sha256sum.txt")
    ):
        errors.append(f"{label}: unsafe path {_display_path(path_text)!r}")
        return None
    return candidate.as_posix()


def _display_path(path_text: str) -> str:
    return SECRET_PATH_REDACTION if SECRET_RE.search(path_text) else path_text


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
    for index, raw_slot_id in enumerate(slot_ids):
        slot_id = raw_slot_id.strip()
        if not slot_id:
            errors.append(f"slot id {index} must be a non-empty string")
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
        normalised.append(candidate.name)
    return normalised, errors


def validate_device_lab_root_path(root: Path) -> list[str]:
    """Validate the device-lab root before slot discovery."""

    if SECRET_RE.search(str(root)):
        return ["device-lab root path must not contain secret-looking material"]
    if root.is_symlink():
        return ["device-lab root must not be a symlink"]
    errors = validate_no_symlink_ancestors(
        root,
        "device-lab root ancestor directory",
    )
    if errors:
        return errors
    if root.exists() and not root.is_dir():
        return ["device-lab root must be a directory"]
    return []


def _reject_secret_slot_path(slot_path: Path, errors: list[str]) -> bool:
    """Reject direct helper calls that receive secret-looking slot paths."""

    if SECRET_RE.search(str(slot_path)):
        errors.append("slot path must not contain secret-looking material")
        return True
    return False


def validate_no_symlink_ancestors(path: Path, label: str) -> list[str]:
    """Reject symlinked parent directories without leaking local paths."""

    candidate = path if path.is_absolute() else Path.cwd() / path
    errors: list[str] = []
    for ancestor in candidate.parents:
        if ancestor.is_absolute() and len(ancestor.parts) <= 2:
            continue
        try:
            if ancestor.is_symlink():
                errors.append(f"{label} must not be a symlink")
                break
        except OSError:
            errors.append(f"{label} metadata could not be read")
            break
        if not ancestor.exists():
            continue
    return errors


def _validate_manifest_slot_path(slot_path: Path) -> list[str]:
    if SECRET_RE.search(str(slot_path)):
        return ["slot path must not contain secret-looking material"]
    if slot_path.is_symlink():
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
    if manifest_path.is_symlink():
        return entries, ["sha256sum.txt must not be a symlink"]
    if manifest_path.exists() and not manifest_path.is_file():
        return entries, ["sha256sum.txt must be a regular file"]
    if not manifest_path.is_file():
        return entries, ["missing sha256sum.txt"]
    try:
        if manifest_path.stat().st_nlink > 1:
            return entries, ["sha256sum.txt must not be hardlinked"]
    except OSError:
        return entries, ["sha256sum.txt hardlink metadata could not be read"]

    lines = manifest_path.read_text(encoding="utf-8").splitlines()
    for line_no, raw in enumerate(lines, start=1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 1)
        if len(parts) != 2:
            errors.append(f"sha256sum.txt line {line_no}: expected '<sha256> <path>'")
            continue
        digest, path_text = parts
        if not SHA256_HEX_RE.fullmatch(digest):
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


def verify_sha256_manifest(slot_path: Path) -> list[str]:
    """Check that sha256sum.txt exactly covers the slot artefacts."""

    root_errors = _validate_manifest_slot_path(slot_path)
    if root_errors:
        return root_errors
    entries, errors = parse_sha256_manifest(slot_path)
    if _has_manifest_file_shape_error(errors):
        return errors
    actual_files = _slot_files(slot_path)

    for relative, expected_digest in sorted(entries.items()):
        path = slot_path / relative
        if _slot_relative_symlink_ancestor(slot_path, relative) is not None:
            errors.append(
                "sha256sum.txt references artifact under symlink directory "
                f"{_display_path(relative)}"
            )
            continue
        if path.is_symlink():
            errors.append(
                f"sha256sum.txt references symlink artifact {_display_path(relative)}"
            )
            continue
        if path.exists() and not path.is_file():
            errors.append(
                f"sha256sum.txt references non-regular artifact {_display_path(relative)}"
            )
            continue
        if path.is_file():
            try:
                link_count = path.stat().st_nlink
            except OSError:
                errors.append(
                    "sha256sum.txt references artifact with unreadable hardlink "
                    f"metadata {_display_path(relative)}"
                )
                continue
            if link_count > 1:
                errors.append(
                    f"sha256sum.txt references hardlinked artifact {_display_path(relative)}"
                )
                continue
        if not path.is_file():
            errors.append(
                f"sha256sum.txt references missing file {_display_path(relative)}"
            )
            continue
        actual_digest = hashlib.sha256(path.read_bytes()).hexdigest()
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
    )


def _read_json_without_duplicate_keys(path: Path) -> Any:
    return _loads_json_without_duplicate_keys(path.read_text(encoding="utf-8"))


def _load_json(path: Path, label: str, errors: list[str]) -> dict[str, Any] | None:
    if SECRET_RE.search(str(path)):
        errors.append(f"{label} path must not contain secret-looking material")
        return None
    json_ancestor_errors = validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if json_ancestor_errors:
        errors.extend(json_ancestor_errors)
        return None
    if path.is_symlink():
        errors.append(f"{label} must not be a symlink")
        return None
    if path.exists() and not path.is_file():
        errors.append(f"{label} must be a regular file")
        return None
    if not path.is_file():
        errors.append(f"missing {label}")
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
        data = _read_json_without_duplicate_keys(path)
    except json.JSONDecodeError as exc:
        errors.append(f"{label} is not valid JSON: {exc}")
        return None
    except DuplicateJsonKeyError as exc:
        errors.append(
            f"{label} contains duplicate JSON object key {_display_path(exc.key)}"
        )
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
    if not isinstance(value, str) or not value.strip():
        errors.append(f"slot.json {key} must be a non-empty string")
        return None
    if SECRET_RE.search(value):
        errors.append(f"slot.json {key} must not contain secret-looking material")
        return None
    return value.strip()


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
    if not isinstance(value, str) or value.strip().lower() not in accepted:
        errors.append(f"slot.json {key} must be one of {sorted(accepted)}")


def _require_evidence_string(
    data: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"signed evidence artifact {key} must be a non-empty string")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"signed evidence artifact {key} must not contain secret-looking material"
        )
        return None
    return value.strip()


def _require_evidence_raw_string(
    data: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not value:
        errors.append(f"signed evidence artifact {key} must be a non-empty string")
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
    if not isinstance(value, str) or not value.strip():
        errors.append(f"attestation/result.json {key} must be a non-empty string")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"attestation/result.json {key} must not contain secret-looking material"
        )
        return None
    return value.strip()


def _attestation_result_matches_slot_metadata(
    result: dict[str, Any],
    metadata: dict[str, Any],
    key: str,
    errors: list[str],
) -> None:
    expected = metadata.get(key)
    actual = _attestation_result_string(result, key, errors)
    if key.endswith("_sha256") and actual is not None and not SHA256_HEX_RE.fullmatch(actual):
        errors.append(f"attestation/result.json {key} must be lowercase sha256 hex")
    if isinstance(expected, str) and actual is not None and actual != expected.strip():
        errors.append(f"attestation/result.json {key} must match slot.json {key}")


def validate_attestation_result(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> None:
    """Validate production StrongBox/KeyMint attestation summary bindings."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    result = _load_json(slot_path / "attestation" / "result.json", "attestation/result.json", errors)
    if result is None:
        return

    for field in sorted(set(result) - ATTESTATION_RESULT_FIELDS):
        errors.append(
            f"attestation/result.json contains unexpected field {_display_path(field)}"
        )

    status = _attestation_result_string(result, "status", errors)
    if status is not None and status.lower() not in {"ok", "passed"}:
        errors.append("attestation/result.json status must be ok or passed")

    slot_bindings: list[str] = []
    for slot_key in ("slot_id", "slot"):
        slot_value = result.get(slot_key)
        if slot_value is None:
            continue
        if not isinstance(slot_value, str) or not slot_value.strip():
            errors.append(f"attestation/result.json {slot_key} must be a non-empty string")
            continue
        if SECRET_RE.search(slot_value):
            errors.append(
                f"attestation/result.json {slot_key} must not contain secret-looking material"
            )
            continue
        slot_binding = slot_value.strip()
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

    security_levels = [
        result.get("keymint_security_level"),
        result.get("attestation_security_level"),
        result.get("keymaster_security_level"),
    ]
    if not any(
        isinstance(level, str) and level.strip().upper() in STRONGBOX_LEVELS
        for level in security_levels
    ):
        errors.append("attestation/result.json must report STRONGBOX security level")

    for key in ATTESTATION_RESULT_SLOT_BINDING_FIELDS:
        _attestation_result_matches_slot_metadata(result, metadata, key, errors)


def _d2d_transcript_string(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not value.strip():
        errors.append(f"d2d payment transcript {key} must be a non-empty string")
        return None
    if SECRET_RE.search(value):
        errors.append(f"d2d payment transcript {key} must not contain secret-looking material")
        return None
    return value.strip()


def _d2d_transcript_sha256(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"d2d payment transcript {key} must be lowercase sha256 hex")
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
    if not isinstance(value, str) or not value.strip():
        errors.append(f"wallet integrity transcript {key} must be a non-empty string")
        return None
    if SECRET_RE.search(value):
        errors.append(
            f"wallet integrity transcript {key} must not contain secret-looking material"
        )
        return None
    return value.strip()


def _wallet_transcript_sha256(
    transcript: dict[str, Any],
    key: str,
    errors: list[str],
) -> str | None:
    value = transcript.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"wallet integrity transcript {key} must be lowercase sha256 hex")
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
) -> None:
    """Validate the offline-offline D2D payment handoff transcript."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    transcript = _load_json(transcript_path, "d2d payment transcript", errors)
    if transcript is None:
        return

    for field in sorted(set(transcript) - D2D_PAYMENT_TRANSCRIPT_FIELDS):
        errors.append(f"d2d payment transcript contains unexpected field {_display_path(field)}")

    if transcript.get("schema") != D2D_PAYMENT_TRANSCRIPT_SCHEMA:
        errors.append(f"d2d payment transcript schema must be {D2D_PAYMENT_TRANSCRIPT_SCHEMA}")

    for key in D2D_PAYMENT_TRANSCRIPT_SLOT_STRING_BINDINGS:
        actual = _d2d_transcript_string(transcript, key, errors)
        expected = metadata.get(key)
        if isinstance(expected, str) and actual is not None and actual != expected.strip():
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
    queue_path = slot_path / "queue" / "pending_queue.json"
    if not queue_path.is_file():
        errors.append("d2d payment transcript queue_after_sha256 requires queue/pending_queue.json")
    elif queue_after_sha256 is not None:
        actual_queue_digest = hashlib.sha256(queue_path.read_bytes()).hexdigest()
        if queue_after_sha256 != actual_queue_digest:
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


def validate_d2d_payment_transcript_binding(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> tuple[str | None, str | None]:
    """Validate the slot.json path/hash binding for the D2D payment transcript."""

    if _reject_secret_slot_path(slot_path, errors):
        return None, None
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
        return None, digest
    if relative.split("/", 1)[0] != "handoff":
        errors.append("slot.json d2d_payment_transcript_path must stay under handoff/")
        return relative, digest

    transcript_path = slot_path / relative
    if not transcript_path.is_file():
        errors.append("slot.json d2d_payment_transcript_path must point to an existing file")
        return None, digest

    matched_digest: str | None = None
    if digest is not None:
        actual_digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
        if actual_digest != digest:
            errors.append(
                "slot.json d2d_payment_transcript_sha256 does not match d2d_payment_transcript_path"
            )
        else:
            matched_digest = digest
    validate_d2d_payment_transcript(slot_path, transcript_path, metadata, errors)
    return relative, matched_digest


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
        if isinstance(expected, str) and actual is not None and actual != expected.strip():
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
    if relative.split("/", 1)[0] != "wallet":
        errors.append("slot.json wallet_integrity_transcript_path must stay under wallet/")
        return relative, digest

    transcript_path = slot_path / relative
    if not transcript_path.is_file():
        errors.append("slot.json wallet_integrity_transcript_path must point to an existing file")
        return relative, digest

    matched_digest: str | None = None
    if digest is not None:
        actual_digest = hashlib.sha256(transcript_path.read_bytes()).hexdigest()
        if actual_digest != digest:
            errors.append(
                "slot.json wallet_integrity_transcript_sha256 does not match wallet_integrity_transcript_path"
            )
        else:
            matched_digest = digest
    validate_wallet_integrity_transcript(transcript_path, metadata, errors)
    return relative, matched_digest


def _require_lowercase_sha256_hex(
    data: dict[str, Any], key: str, label: str, errors: list[str]
) -> str | None:
    value = data.get(key)
    if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value):
        errors.append(f"{label} {key} must be lowercase sha256 hex")
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
    return json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")


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
    return completed.stdout


def _validate_public_key_path_shape(
    public_key_path: Path,
    *,
    errors: list[str],
    label: str,
) -> bool:
    """Reject public key paths that could alias external key material."""

    if SECRET_RE.search(str(public_key_path)):
        errors.append(f"{label} path must not contain secret-looking material")
        return False
    if public_key_path.is_symlink():
        errors.append(f"{label} must not be a symlink")
        return False
    ancestor_errors = validate_no_symlink_ancestors(
        public_key_path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        errors.extend(ancestor_errors)
        return False
    if public_key_path.exists() and not public_key_path.is_file():
        errors.append(f"{label} must be a regular file")
        return False
    if not public_key_path.is_file():
        errors.append(f"{label} must point to an existing public key file")
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


def load_trusted_signer_public_keys(
    public_key_paths: Iterable[str | Path] | None,
) -> tuple[dict[str, Path], list[str]]:
    """Load trusted lab signer public keys and return them keyed by DER SHA-256."""

    errors: list[str] = []
    trusted: dict[str, Path] = {}
    for raw_path in public_key_paths or []:
        path = Path(raw_path)
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


def _verify_ed25519_signature(
    *,
    public_key_path: Path,
    payload: bytes,
    signature: bytes,
    errors: list[str],
    label: str = "trusted signer public key",
) -> None:
    openssl = _require_openssl(errors)
    if openssl is None:
        return
    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):
        return
    with tempfile.TemporaryDirectory(prefix="iroha-kagemusha-evidence-") as temp:
        temp_path = Path(temp)
        payload_path = temp_path / "payload.bin"
        signature_path = temp_path / "signature.bin"
        payload_path.write_bytes(payload)
        signature_path.write_bytes(signature)
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


def _required_signed_evidence_digest_paths(slot_path: Path) -> list[str]:
    return sorted(
        {
            relative
            for relative in _slot_files(slot_path)
            if relative.split("/", 1)[0] in set(EXPECTED_DIRS) | {"handoff", "wallet"}
        }
        | set(REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS)
    )


def validate_required_kagemusha_slot_artifact_shapes(
    slot_path: Path, errors: list[str]
) -> None:
    """Validate base production slot artifacts before they are signed or accepted."""

    if _reject_secret_slot_path(slot_path, errors):
        return
    for relative in REQUIRED_KAGEMUSHA_SLOT_ARTIFACT_PATHS:
        artifact_path = slot_path / relative
        if not artifact_path.is_file():
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

    _validate_required_telemetry_artifact(slot_path, errors)
    _validate_required_status_artifact(slot_path, errors)
    _validate_required_runtime_log_artifact(slot_path, errors)


def _validate_required_telemetry_artifact(slot_path: Path, errors: list[str]) -> None:
    telemetry = _load_json(
        slot_path / "telemetry" / "telemetry.json",
        "telemetry/telemetry.json",
        errors,
    )
    if telemetry is None:
        return
    if telemetry.get("schema_version") != 1:
        errors.append("telemetry/telemetry.json schema_version must be 1")
    slot_id = telemetry.get("slot_id")
    if not isinstance(slot_id, str) or slot_id.strip() != slot_path.name:
        errors.append("telemetry/telemetry.json slot_id must match the slot directory name")
    suite = telemetry.get("suite")
    if not isinstance(suite, str) or not suite.strip():
        errors.append("telemetry/telemetry.json suite must be a non-empty string")
    elif "kagemusha" not in suite.lower():
        errors.append("telemetry/telemetry.json suite must identify a Kagemusha device-lab run")


def _validate_required_status_artifact(slot_path: Path, errors: list[str]) -> None:
    status_path = slot_path / "telemetry" / "status.ndjson"
    if not status_path.is_file():
        return
    try:
        lines = status_path.read_text(encoding="utf-8").splitlines()
    except OSError:
        errors.append("telemetry/status.ndjson could not be read")
        return

    saw_record = False
    saw_ok = False
    for line_no, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()
        if not line:
            continue
        saw_record = True
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
        status = status_entry.get("status")
        if not isinstance(status, str) or not status.strip():
            errors.append(f"telemetry/status.ndjson line {line_no} status must be a non-empty string")
            continue
        normalized = status.strip().lower()
        if normalized == "ok":
            saw_ok = True
        elif normalized in KAGEMUSHA_STATUS_FAILURE_VALUES:
            errors.append(
                f"telemetry/status.ndjson line {line_no} status must not be {status!r}"
            )

    if not saw_record:
        errors.append("telemetry/status.ndjson must contain at least one JSON status record")
    elif not saw_ok:
        errors.append("telemetry/status.ndjson must contain at least one ok status")


def _validate_required_runtime_log_artifact(slot_path: Path, errors: list[str]) -> None:
    log_path = slot_path / "logs" / "runtime.log"
    if not log_path.is_file():
        return
    try:
        text = log_path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        errors.append("logs/runtime.log could not be read")
        return
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
        if isinstance(expected, str) and value is not None and value != expected.strip():
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
            if SECRET_RE.search(command):
                errors.append(
                    f"signed evidence artifact raw_test_commands[{index}] must not contain secret-looking material"
                )
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
        expected_len=64,
        label="signed evidence artifact signature",
        errors=errors,
    )

    payload = _canonical_signed_evidence_payload(evidence)
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

    validate_required_kagemusha_slot_artifact_shapes(slot_path, errors)

    required_paths = _required_signed_evidence_digest_paths(slot_path)
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
        artifact_path = slot_path / relative
        if not artifact_path.is_file():
            errors.append(
                "signed evidence artifact required slot artifact is missing "
                f"{_display_path(relative)}"
            )
            continue
        actual_digest = hashlib.sha256(artifact_path.read_bytes()).hexdigest()
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
    if _reject_secret_slot_path(slot_path, errors):
        return errors, details
    metadata = _load_json(slot_path / "slot.json", "slot.json", errors)
    if metadata is None:
        return errors, details

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
        if chain_relative.split("/", 1)[0] != "attestation":
            errors.append(
                "slot.json attestation_certificate_chain_path must stay under attestation/"
            )
        else:
            chain_path = slot_path / chain_relative
            if not chain_path.is_file():
                errors.append(
                    "slot.json attestation_certificate_chain_path must point to an existing file"
                )
            else:
                chain_bytes = chain_path.read_bytes()
                _validate_attestation_certificate_chain_artifact(
                    chain_relative,
                    chain_bytes,
                    errors,
                )
                actual_chain_digest = hashlib.sha256(chain_bytes).hexdigest()
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
        apk_path = slot_path / apk_relative
        if not apk_path.is_file():
            errors.append("slot.json offline_wallet_apk_path must point to an existing file")
        elif apk_digest is not None:
            actual_apk_digest = hashlib.sha256(apk_path.read_bytes()).hexdigest()
            if actual_apk_digest != apk_digest:
                errors.append(
                    "slot.json offline_wallet_apk_sha256 does not match offline_wallet_apk_path"
                )
            else:
                details["offline_wallet_apk_path"] = apk_relative
                details["offline_wallet_apk_sha256"] = apk_digest

    d2d_relative, d2d_digest = validate_d2d_payment_transcript_binding(
        slot_path,
        metadata,
        errors,
    )
    if d2d_relative is not None and d2d_digest is not None:
        details["d2d_payment_transcript_path"] = d2d_relative
        details["d2d_payment_transcript_sha256"] = d2d_digest

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
    _require_status(metadata, "abi6_recursive_spend_jni_probe", {"passed", "ok"}, errors)
    _require_status(
        metadata,
        "abi7_recursive_compact_jni_probe",
        {"unavailable", "fail_closed"},
        errors,
    )
    _require_status(
        metadata,
        "abi7_recursive_compact_prover_state",
        {"unavailable", "proof_composition_unavailable", "fail_closed"},
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

    security_level = _require_non_empty_string(metadata, "keymint_security_level", errors)
    if security_level is not None and security_level.upper() not in STRONGBOX_LEVELS:
        errors.append("slot.json keymint_security_level must be STRONGBOX")
    validate_attestation_result(slot_path, metadata, errors)

    digest = _require_non_empty_string(metadata, "signed_evidence_artifact_sha256", errors)
    if digest is not None and not SHA256_HEX_RE.fullmatch(digest):
        errors.append("slot.json signed_evidence_artifact_sha256 must be lowercase sha256 hex")
    artifact_relative = _require_non_empty_string(
        metadata, "signed_evidence_artifact_path", errors
    )
    if artifact_relative is not None:
        artifact_relative = _normalise_safe_relative_path(
            artifact_relative,
            errors,
            "slot.json signed_evidence_artifact_path",
        )
    artifact_root_ok = True
    if artifact_relative is not None and artifact_relative.split("/", 1)[0] != "evidence":
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
        if not artifact_path.is_file():
            errors.append(
                "slot.json signed_evidence_artifact_path must point to an existing file"
            )
        elif digest is not None and SHA256_HEX_RE.fullmatch(digest):
            actual_digest = hashlib.sha256(artifact_path.read_bytes()).hexdigest()
            if actual_digest != digest:
                errors.append(
                    "slot.json signed_evidence_artifact_sha256 does not match signed_evidence_artifact_path"
                )
            else:
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
            if SECRET_RE.search(command):
                errors.append(
                    f"slot.json raw_test_commands[{index}] must not contain secret-looking material"
                )
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
    slot_label = _display_path(slot_path.name)

    if SECRET_RE.search(slot_path.name):
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory name must not contain secret-looking material"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    if slot_path.is_symlink():
        return {
            "slot": slot_label,
            "status": "error",
            "errors": ["slot directory must not be a symlink"],
            "present": present,
            "file_counts": file_counts,
            "kagemusha": {"required": require_kagemusha_production_evidence},
        }

    if slot_path.parent.is_symlink():
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

    if not slot_path.is_dir():
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
        exists = dir_path.is_dir()
        present[dirname] = exists
        if not exists:
            errors.append(f"missing {dirname}/ directory")
            continue
        count = sum(1 for entry in dir_path.rglob("*") if entry.is_file())
        file_counts[dirname] = count
        if count == 0:
            errors.append(f"{dirname}/ contains no files")

    sha_path = slot_path / "sha256sum.txt"
    present["sha256sum.txt"] = sha_path.is_file()
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


def discover_slots(root: Path, slot_ids: Iterable[str] | None) -> List[Path]:
    """List slot directories under the given root."""
    if slot_ids is not None:
        return [root / slot for slot in slot_ids]
    return [p for p in root.iterdir() if p.is_dir()]


def build_summary(
    root: Path,
    reports: list[dict],
    *,
    require_kagemusha_production_evidence: bool = False,
    require_kagemusha_standard_matrix: bool = False,
    trusted_signer_public_keys: dict[str, Path] | None = None,
) -> dict:
    now = dt.datetime.now(dt.timezone.utc).replace(microsecond=0)
    summary = {
        "schema_version": 1,
        "generated_at": now.isoformat().replace("+00:00", "Z"),
        "root": DEVICE_LAB_ROOT_SUMMARY_LABEL,
        "slots": reports,
        "ok": sum(1 for r in reports if r["status"] == "ok"),
        "failed": sum(1 for r in reports if r["status"] != "ok"),
    }
    if require_kagemusha_production_evidence or require_kagemusha_standard_matrix:
        covered = sorted(
            {
                report.get("kagemusha", {}).get("device_family")
                for report in reports
                if report.get("status") == "ok"
                and report.get("kagemusha", {}).get("device_family") is not None
            }
        )
        missing = [
            family
            for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            if family not in covered
        ]
        summary["kagemusha"] = {
            "production_evidence_required": require_kagemusha_production_evidence,
            "standard_matrix_required": require_kagemusha_standard_matrix,
            "required_device_families": list(KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "covered_device_families": covered,
            "missing_device_families": missing,
            "trusted_signer_public_key_sha256": sorted(
                (trusted_signer_public_keys or {}).keys()
            ),
        }
    return summary


def validate_summary_output_path(path: Path, label: str) -> list[str]:
    """Reject summary output paths that could overwrite aliased local files."""

    if SECRET_RE.search(str(path)):
        return [f"{label} must not contain secret-looking material"]
    ancestor_errors = validate_no_symlink_ancestors(
        path,
        f"{label} ancestor directory",
    )
    if ancestor_errors:
        return ancestor_errors
    parent = path.parent
    if parent.exists():
        if parent.is_symlink():
            return [f"{label} parent directory must not be a symlink"]
        if not parent.is_dir():
            return [f"{label} parent must be a directory"]
    else:
        try:
            parent.mkdir(parents=True, exist_ok=True)
        except OSError:
            return [f"{label} parent directory could not be created"]
    if path.exists() or path.is_symlink():
        if path.is_symlink():
            return [f"{label} must not be a symlink"]
        if not path.is_file():
            return [f"{label} must be a regular file"]
        try:
            link_count = path.stat().st_nlink
        except OSError:
            return [f"{label} hardlink metadata could not be read"]
        if link_count > 1:
            return [f"{label} must not be hardlinked"]
    return []


def write_summary(path: Path, summary: dict) -> list[str]:
    errors = validate_summary_output_path(path, "--json-out")
    if errors:
        return errors
    try:
        path.write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    except OSError:
        return ["--json-out could not be written"]
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
        help="Require production evidence for every standard Kagemusha device family.",
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
    if args.json_out is not None and SECRET_RE.search(args.json_out):
        path_arg_errors.append("--json-out must not contain secret-looking material")
    if path_arg_errors:
        for error in path_arg_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1

    root = Path(args.root)
    root_errors = validate_device_lab_root_path(root)
    if root_errors:
        for error in root_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1
    if not root.exists():
        if args.allow_missing_root:
            print("[device-lab] root missing; skipping")
            return 0
        print("[device-lab] root does not exist", file=sys.stderr)
        return 1

    slot_ids, slot_id_errors = validate_slot_ids(args.slots)
    if slot_id_errors:
        for error in slot_id_errors:
            print(f"[device-lab] {error}", file=sys.stderr)
        return 1

    slot_paths = discover_slots(root, slot_ids)
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
        covered = {
            report.get("kagemusha", {}).get("device_family")
            for report in reports
            if report.get("status") == "ok"
        }
        missing = [
            family
            for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
            if family not in covered
        ]
        if missing:
            failures += 1
            print(
                "[device-lab] missing Kagemusha production evidence for device families: "
                + ", ".join(missing),
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
