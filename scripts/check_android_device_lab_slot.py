"""Validate Android device-lab slots for AND6 compliance evidence."""

from __future__ import annotations

import argparse
import base64
from collections.abc import Mapping
import datetime as dt
import hashlib
import json
import math
import os
from pathlib import Path
from pathlib import PurePosixPath
import re
import stat
import struct
import subprocess
import sys
import tempfile
from typing import Any, Iterable
import unicodedata
import zipfile

try:
    from scripts.android_device_lab_candidate_stage import (
        CandidateStageContract,
        validate_candidate_stage_manifest_v2 as _validate_candidate_stage_manifest_v2,
    )
except ModuleNotFoundError:  # Direct execution places scripts/ on sys.path.
    from android_device_lab_candidate_stage import (
        CandidateStageContract,
        validate_candidate_stage_manifest_v2 as _validate_candidate_stage_manifest_v2,
    )


EXPECTED_DIRS: tuple[str, ...] = ("telemetry", "attestation", "queue", "logs")
OPTIONAL_EVIDENCE_DIRS: tuple[str, ...] = ("evidence", "handoff", "wallet", "scenario")
FORBIDDEN_OPENSSL_CHILD_ENV_KEYS: frozenset[str] = frozenset(
    (
        "OPENSSL_CONF",
        "OPENSSL_CONF_INCLUDE",
        "OPENSSL_MODULES",
        "OPENSSL_ENGINES",
        "OPENSSL_TRACE",
        "OPENSSL_DEBUG_MEMORY",
        "RANDFILE",
        "LD_PRELOAD",
        "LD_LIBRARY_PATH",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_LIBRARY_PATH",
    )
)
ANDROID_KEY_ATTESTATION_EXTENSION_OID = "1.3.6.1.4.1.11129.2.1.17"
ANDROID_KEY_ATTESTATION_EXTENSION_OID_DER = bytes.fromhex(
    "060a2b06010401d679020111"
)
KAGEMUSHA_WALLET_PACKAGE_NAME = "org.hyperledger.iroha.kagemushawallet"
ANDROID_SECURITY_LEVEL_STRONGBOX = 2
ANDROID_VERIFIED_BOOT_STATE_VERIFIED = 0
ANDROID_TAG_ALL_APPLICATIONS = 600
ANDROID_TAG_ROOT_OF_TRUST = 704
ANDROID_TAG_ATTESTATION_APPLICATION_ID = 709
MAX_ANDROID_ATTESTATION_REVOCATION_STATUS_BYTES = 1024 * 1024
MAX_AUTHORITY_TOOL_BYTES = 256 * 1024 * 1024

# Set only after all paths, metadata, and caller-supplied digests have been
# checked. The command-line entry point requires an explicit configuration for
# production evidence. Tests and other in-process callers use the same public
# configurator; there is deliberately no PATH or SDK-directory discovery.
_ANDROID_EVIDENCE_AUTHORITY: dict[str, Any] | None = None
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
KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH = "evidence/candidate-binding-v2.json"
KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH = "evidence/lifecycle-transcript-v2.json"
KAGEMUSHA_STRONGBOX_CHALLENGE_DOMAIN_V1: bytes = (
    b"IROHA_KAGEMUSHA_STRONGBOX_CHALLENGE_V1\x00"
)
KAGEMUSHA_STRONGBOX_CHALLENGE_FIELDS_V1: tuple[str, ...] = (
    "slot_id",
    "candidate_record_sha256",
    "candidate_manifest_sha256",
    "candidate_stage_manifest_sha256",
    "candidate_lab_native_library_sha256",
    "candidate_lab_apk_sha256",
    "candidate_lab_test_apk_sha256",
    "candidate_source_commit",
    "candidate_source_tree_sha256",
)
KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2 = "candidate-stage-manifest-v2.json"
KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_SCHEMA_V2 = (
    "iroha.kagemusha.android_candidate_stage_manifest.v2"
)
KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_FIELDS_V2: frozenset[str] = frozenset(
    {
        "schema",
        "version",
        "stage_manifest_path",
        "stage_manifest_mode",
        "stage_manifest_size_bytes",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_validation_report_sha256",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "scenario_inventory_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "validator",
        "entry_count",
        "scenario_entry_count",
        "entries",
    }
)
KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_PATH_V2 = (
    "evidence/candidate/candidate-validation-v2.json"
)
KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_SCHEMA_V2 = (
    "iroha.kagemusha.recursive_spend.candidate_validation.v2"
)
KAGEMUSHA_QUALIFICATION_RECEIPT_FILE_NAME_V4 = (
    "recursive-step-two-qualification-v4.norito"
)
KAGEMUSHA_QUALIFIED_CANDIDATE_DOMAIN_V4 = (
    b"iroha:kagemusha:recursive-spend-qualified-candidate:v4"
)
KAGEMUSHA_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V1 = "self-physical-footprint-v1"
KAGEMUSHA_GENERATION_MEMORY_LIMIT_MAX_BYTES = 64 * 1024 * 1024 * 1024
KAGEMUSHA_CANDIDATE_VALIDATION_FIELDS_V2: frozenset[str] = frozenset(
    {
        "schema",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "qualification_receipt_file_name",
        "qualification_receipt_sha256",
        "qualified_candidate_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "generation",
        "generation_memory_limit_bytes",
        "generation_memory_enforcement_profile",
        "bridge_abi_version",
        "artifact_count",
        "artifacts",
        "topup_finality_roster_file_name",
        "topup_finality_roster_size_bytes",
        "topup_finality_roster_sha256",
    }
)
KAGEMUSHA_CANDIDATE_STAGE_ENTRY_FIELDS_V1: frozenset[str] = frozenset(
    {"path", "mode", "size_bytes", "sha256"}
)
KAGEMUSHA_CANDIDATE_STAGE_VALIDATOR_FIELDS_V1: frozenset[str] = frozenset(
    {
        "schema",
        "candidate_binary_name",
        "candidate_binary_sha256",
        "scenario_binary_name",
        "scenario_binary_sha256",
        "cargo_binary_sha256",
        "cargo_version_verbose",
        "rustc_binary_sha256",
        "rustc_version_verbose",
        "locked",
        "offline",
        "isolated_target",
        "build_jobs",
        "candidate_package",
        "scenario_package",
        "features",
        "profile",
    }
)
KAGEMUSHA_CANDIDATE_STAGE_VALIDATOR_SCHEMA_V1 = (
    "iroha.kagemusha.android_candidate_validator.v1"
)
KAGEMUSHA_CANDIDATE_SCENARIO_INVENTORY_DOMAIN_V1: bytes = (
    b"iroha.kagemusha.android-candidate-scenario-inventory.v1\x00"
)
KAGEMUSHA_CANDIDATE_SCENARIO_FILES_V1: tuple[str, ...] = (
    "init-top-up-anchor-v4.norito",
    "init-top-up-finality-proof-v2.norito",
    "init-top-up-finality-roster-artifact-v2.norito",
    "init-opening-v2.norito",
    "init-output-membership-v4.norito",
    "transfer-verifier-commitment-v2.bin",
    "append-hop-01-recipient-request-v2.norito",
    "append-hop-01-recipient-opening-v2.norito",
    "append-hop-01-change-opening-v2.norito",
    "append-hop-01-output-membership-v4.norito",
    "append-hop-01-operation-id.bin",
    "append-hop-01-block-height.txt",
    "append-hop-01-verified-at-ms.txt",
    "append-hop-02-recipient-request-v2.norito",
    "append-hop-02-recipient-opening-v2.norito",
    "append-hop-02-change-opening-v2.norito",
    "append-hop-02-output-membership-v4.norito",
    "append-hop-02-operation-id.bin",
    "append-hop-02-block-height.txt",
    "append-hop-02-verified-at-ms.txt",
    "redeem-recipient-account-id.txt",
    "unshield-verifier-commitment-v2.bin",
    "redeem-hop-01-operation-id.bin",
    "redeem-hop-01-block-height.txt",
    "redeem-hop-02-operation-id.bin",
    "redeem-hop-02-block-height.txt",
    "redeem-sender-change-operation-id.bin",
    "redeem-sender-change-block-height.txt",
    "duplicate-input-recipient-request-v2.norito",
    "duplicate-input-output-membership-v4.norito",
    "duplicate-input-operation-id.bin",
    "duplicate-input-block-height.txt",
    "duplicate-input-verified-at-ms.txt",
)
MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_KAGEMUSHA_WALLET_APK_BYTES = 64 * 1024 * 1024
MAX_KAGEMUSHA_CANDIDATE_NATIVE_LIBRARY_BYTES = 256 * 1024 * 1024
MAX_KAGEMUSHA_KRV4_ARTIFACT_BYTES = 5 * 1024 * 1024 * 1024
MAX_KAGEMUSHA_KRV4_HEADER_BYTES = 64 * 1024
MAX_ANDROID_DEVICE_LAB_JSON_BYTES = 16 * 1024 * 1024
KAGEMUSHA_WALLET_APK_PATH = "evidence/kagemusha-wallet-release.apk"
MAX_ANDROID_DEVICE_LAB_SHA256_MANIFEST_BYTES = 1024 * 1024
MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES = 64 * 1024
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


def derive_kagemusha_qualified_candidate_sha256_v4(
    candidate_record_sha256: str,
    qualification_receipt_sha256: str,
) -> str:
    """Derive the exact domain-separated qualified-candidate identity."""

    for label, value in (
        ("candidate record", candidate_record_sha256),
        ("qualification receipt", qualification_receipt_sha256),
    ):
        if not isinstance(value, str) or not SHA256_HEX_RE.fullmatch(value) or value == "0" * 64:
            raise ValueError(f"{label} digest must be non-zero lowercase SHA-256")
    digest = hashlib.sha256()
    digest.update(KAGEMUSHA_QUALIFIED_CANDIDATE_DOMAIN_V4)
    digest.update(b"\0")
    digest.update(bytes.fromhex(candidate_record_sha256))
    digest.update(bytes.fromhex(qualification_receipt_sha256))
    return digest.hexdigest()


def derive_kagemusha_strongbox_challenge_v1(metadata: Mapping[str, Any]) -> bytes:
    """Derive the exact 32-byte candidate-stage StrongBox challenge."""

    digest = hashlib.sha256()
    digest.update(KAGEMUSHA_STRONGBOX_CHALLENGE_DOMAIN_V1)
    for field in KAGEMUSHA_STRONGBOX_CHALLENGE_FIELDS_V1:
        value = metadata.get(field)
        if not isinstance(value, str) or not value or value != value.strip():
            raise ValueError(f"{field} must be one exact non-empty string")
        if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
            raise ValueError(f"{field} must not contain control characters")
        field_bytes = field.encode("utf-8")
        value_bytes = value.encode("utf-8")
        if len(field_bytes) > 0xFFFFFFFF or len(value_bytes) > 0xFFFFFFFF:
            raise ValueError(f"{field} exceeds the u32 framing limit")
        digest.update(len(field_bytes).to_bytes(4, "big"))
        digest.update(field_bytes)
        digest.update(len(value_bytes).to_bytes(4, "big"))
        digest.update(value_bytes)
    return digest.digest()


def validate_kagemusha_candidate_stage_manifest_v2(
    stage_root: Path,
    *,
    candidate_sha256: str,
    stage_sha256: str,
    source_commit: str,
    source_tree_sha256: str,
    verify_entry_digests: bool = True,
) -> dict[str, Any]:
    """Verify the canonical candidate-stage manifest and its exact inventory."""

    contract = CandidateStageContract(
        stage_manifest_path=KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2,
        stage_manifest_schema=KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_SCHEMA_V2,
        stage_manifest_fields=KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_FIELDS_V2,
        validation_report_path=KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_PATH_V2,
        validation_report_schema=KAGEMUSHA_CANDIDATE_VALIDATION_REPORT_SCHEMA_V2,
        validation_report_fields=KAGEMUSHA_CANDIDATE_VALIDATION_FIELDS_V2,
        qualification_receipt_file_name=KAGEMUSHA_QUALIFICATION_RECEIPT_FILE_NAME_V4,
        generation_memory_enforcement_profile=(
            KAGEMUSHA_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V1
        ),
        generation_memory_limit_max_bytes=(
            KAGEMUSHA_GENERATION_MEMORY_LIMIT_MAX_BYTES
        ),
        stage_entry_fields=KAGEMUSHA_CANDIDATE_STAGE_ENTRY_FIELDS_V1,
        stage_validator_fields=KAGEMUSHA_CANDIDATE_STAGE_VALIDATOR_FIELDS_V1,
        stage_validator_schema=KAGEMUSHA_CANDIDATE_STAGE_VALIDATOR_SCHEMA_V1,
        scenario_inventory_domain=KAGEMUSHA_CANDIDATE_SCENARIO_INVENTORY_DOMAIN_V1,
        scenario_files=KAGEMUSHA_CANDIDATE_SCENARIO_FILES_V1,
        artifact_roles=KAGEMUSHA_CANDIDATE_ARTIFACT_ROLES_V4,
        artifact_file_names=KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
        max_json_bytes=MAX_ANDROID_DEVICE_LAB_JSON_BYTES,
        derive_qualified_candidate_sha256=(
            derive_kagemusha_qualified_candidate_sha256_v4
        ),
    )
    return _validate_candidate_stage_manifest_v2(
        stage_root,
        contract=contract,
        candidate_sha256=candidate_sha256,
        stage_sha256=stage_sha256,
        source_commit=source_commit,
        source_tree_sha256=source_tree_sha256,
        verify_entry_digests=verify_entry_digests,
    )


def extract_apk_signing_certificate_sha256(apk_path: Path) -> str:
    """Verify APK v2/v3 signatures and return the sole signer DER digest.

    The Android SDK ``apksigner`` launcher is a mutable shell wrapper around an
    ambient Java runtime and a sibling jar. Neither dependency is identified by
    the launcher's digest. The authority contract therefore admits the Java
    executable and ``apksigner.jar`` themselves and invokes that exact pair.
    """

    signing_scheme_ids = {0x7109871A, 0xF05368C0, 0x1B93AD61}

    def take_length_prefixed(payload: bytes, offset: int, label: str) -> tuple[bytes, int]:
        if offset + 4 > len(payload):
            raise ValueError(f"APK signing block truncates {label} length")
        length = int.from_bytes(payload[offset : offset + 4], "little")
        start = offset + 4
        end = start + length
        if length <= 0 or end > len(payload):
            raise ValueError(f"APK signing block has invalid {label} length")
        return payload[start:end], end

    def require_der_certificate(payload: bytes) -> None:
        if len(payload) < 4 or payload[0] != 0x30:
            raise ValueError("APK signer certificate is not a DER SEQUENCE")
        first_length = payload[1]
        if first_length < 0x80:
            header_length = 2
            content_length = first_length
        else:
            length_octets = first_length & 0x7F
            if length_octets == 0 or length_octets > 4 or 2 + length_octets > len(payload):
                raise ValueError("APK signer certificate DER length is invalid")
            if payload[2] == 0:
                raise ValueError("APK signer certificate DER length is non-minimal")
            header_length = 2 + length_octets
            content_length = int.from_bytes(payload[2:header_length], "big")
            if content_length < 0x80:
                raise ValueError("APK signer certificate DER length is non-minimal")
        if header_length + content_length != len(payload):
            raise ValueError("APK signer certificate DER is truncated or has trailing bytes")

    path = apk_path.resolve()
    file_stat = path.stat()
    if not stat.S_ISREG(file_stat.st_mode) or file_stat.st_size < 64:
        raise ValueError("APK must be one non-empty regular file")
    java, java_sha256 = _configured_authority_tool("java")
    apksigner_jar, apksigner_jar_sha256 = _configured_authority_tool(
        "apksigner_jar"
    )
    checked_java, _, java_errors = _read_pinned_authority_file(
        java,
        java_sha256,
        label="configured Java executable",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    checked_jar, _, jar_errors = _read_pinned_authority_file(
        apksigner_jar,
        apksigner_jar_sha256,
        label="configured apksigner.jar",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
    )
    if checked_java is None or checked_jar is None or java_errors or jar_errors:
        raise ValueError(
            "configured Java/apksigner.jar no longer matches its authority pin"
        )
    verifier_env = {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin:/bin",
    }
    verified = subprocess.run(
        [
            os.fspath(java),
            "-jar",
            os.fspath(apksigner_jar),
            "verify",
            "--verbose",
            "--print-certs",
            str(path),
        ],
        capture_output=True,
        text=True,
        env=verifier_env,
        check=False,
    )
    checked_java, _, java_errors = _read_pinned_authority_file(
        java,
        java_sha256,
        label="configured Java executable",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    checked_jar, _, jar_errors = _read_pinned_authority_file(
        apksigner_jar,
        apksigner_jar_sha256,
        label="configured apksigner.jar",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
    )
    if checked_java is None or checked_jar is None or java_errors or jar_errors:
        raise ValueError("configured Java/apksigner.jar changed during verification")
    if verified.returncode != 0:
        raise ValueError("apksigner cryptographic verification failed")
    verifier_digests = re.findall(
        r"^Signer #[0-9]+ certificate SHA-256 digest: ([0-9A-Fa-f:]{64,95})$",
        verified.stdout,
        flags=re.MULTILINE,
    )
    normalized_verifier_digests = {
        digest.replace(":", "").lower() for digest in verifier_digests
    }
    if len(normalized_verifier_digests) != 1:
        raise ValueError("apksigner must report exactly one current signer digest")
    with path.open("rb") as handle:
        tail_size = min(file_stat.st_size, 22 + 0xFFFF)
        handle.seek(file_stat.st_size - tail_size)
        tail = handle.read(tail_size)
        eocd_offset = tail.rfind(b"PK\x05\x06")
        if eocd_offset < 0 or eocd_offset + 22 > len(tail):
            raise ValueError("APK has no complete ZIP end-of-central-directory record")
        comment_length = int.from_bytes(tail[eocd_offset + 20 : eocd_offset + 22], "little")
        if eocd_offset + 22 + comment_length != len(tail):
            raise ValueError("APK ZIP end-of-central-directory record is not final")
        central_offset = int.from_bytes(tail[eocd_offset + 16 : eocd_offset + 20], "little")
        if central_offset in (0, 0xFFFFFFFF) or central_offset < 24:
            raise ValueError("APK ZIP central-directory offset is unsupported")
        handle.seek(central_offset - 24)
        footer = handle.read(24)
        if len(footer) != 24 or footer[8:] != b"APK Sig Block 42":
            raise ValueError("APK has no v2/v3 signing block")
        block_size = struct.unpack_from("<Q", footer, 0)[0]
        total_size = block_size + 8
        if block_size < 24 or total_size > central_offset or total_size > 64 * 1024 * 1024:
            raise ValueError("APK signing block size is invalid")
        block_start = central_offset - total_size
        handle.seek(block_start)
        block = handle.read(total_size)
    if len(block) != total_size or struct.unpack_from("<Q", block, 0)[0] != block_size:
        raise ValueError("APK signing block header/footer sizes differ")
    pairs = block[8:-24]
    pair_offset = 0
    signer_certificates: set[bytes] = set()
    while pair_offset < len(pairs):
        if pair_offset + 8 > len(pairs):
            raise ValueError("APK signing block truncates an ID-value pair")
        pair_size = struct.unpack_from("<Q", pairs, pair_offset)[0]
        pair_start = pair_offset + 8
        pair_end = pair_start + pair_size
        if pair_size < 4 or pair_end > len(pairs):
            raise ValueError("APK signing block ID-value pair size is invalid")
        pair_id = struct.unpack_from("<I", pairs, pair_start)[0]
        if pair_id in signing_scheme_ids:
            scheme_block = pairs[pair_start + 4 : pair_end]
            signers, scheme_end = take_length_prefixed(
                scheme_block, 0, "signers sequence"
            )
            if scheme_end != len(scheme_block):
                raise ValueError("APK signing scheme block has trailing bytes")
            signer_offset = 0
            while signer_offset < len(signers):
                signer, signer_offset = take_length_prefixed(
                    signers, signer_offset, "signer"
                )
                signed_data, _ = take_length_prefixed(signer, 0, "signed data")
                _, signed_offset = take_length_prefixed(signed_data, 0, "digests")
                certificates, _ = take_length_prefixed(
                    signed_data, signed_offset, "certificates"
                )
                certificate, certificate_end = take_length_prefixed(
                    certificates, 0, "certificate"
                )
                if certificate_end != len(certificates):
                    raise ValueError("APK signer must contain exactly one certificate")
                require_der_certificate(certificate)
                signer_certificates.add(certificate)
        pair_offset = pair_end
    if pair_offset != len(pairs) or len(signer_certificates) != 1:
        raise ValueError("APK must expose exactly one current v2/v3 signer certificate")
    measured = hashlib.sha256(signer_certificates.pop()).hexdigest()
    if normalized_verifier_digests != {measured}:
        raise ValueError("apksigner digest differs from parsed signer certificate DER")
    return measured


def _candidate_lab_apk_forbidden_krv4_entries(apk_path: Path) -> list[str]:
    """Return canonical KRV4 basenames found in a candidate lab APK directory."""

    forbidden_names = set(KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4)
    try:
        with zipfile.ZipFile(apk_path) as archive:
            return sorted(
                {
                    entry.filename
                    for entry in archive.infolist()
                    if PurePosixPath(entry.filename.replace("\\", "/")).name
                    in forbidden_names
                },
                key=lambda value: value.encode("utf-8"),
            )
    except (OSError, zipfile.BadZipFile, zipfile.LargeZipFile) as error:
        raise ValueError("candidate lab APK is not a readable bounded ZIP archive") from error


STATUS_EVENT_FIELDS: frozenset[str] = frozenset(
    {
        "status",
        "slot_id",
    }
)


def _slot_artifact_max_bytes(relative: str) -> int:
    if relative == KAGEMUSHA_WALLET_APK_PATH:
        return MAX_KAGEMUSHA_WALLET_APK_BYTES
    if relative.endswith(".apk") and _safe_relative_path_is_child_of(relative, "evidence"):
        return MAX_KAGEMUSHA_WALLET_APK_BYTES
    if relative.endswith(".so") and _safe_relative_path_is_child_of(relative, "evidence"):
        return MAX_KAGEMUSHA_CANDIDATE_NATIVE_LIBRARY_BYTES
    if relative.endswith(".krv4") and _safe_relative_path_is_child_of(relative, "evidence"):
        return MAX_KAGEMUSHA_KRV4_ARTIFACT_BYTES
    return MAX_KAGEMUSHA_REQUIRED_SLOT_ARTIFACT_BYTES
DEVICE_LAB_ROOT_SUMMARY_LABEL = "<local-device-lab-root>"
SUMMARY_REDACTION_KEY_COLLISION_FIELD = "summary_redaction_key_collision"
SUMMARY_NON_STRING_KEY_NORMALIZED_FIELD = "summary_non_string_key_normalized"
SUMMARY_NON_STRING_KEY_REDACTION = "<non-string-summary-key>"
SUMMARY_NONFINITE_NUMBER_NORMALIZED_FIELD = "summary_nonfinite_number_normalized"
SUMMARY_NONFINITE_NUMBER_REDACTION = "<non-finite-summary-number>"
JSON_NONFINITE_CONSTANT_REDACTION = "<non-finite-json-constant>"
SUMMARY_UNSUPPORTED_VALUE_NORMALIZED_FIELD = "summary_unsupported_value_normalized"
SUMMARY_UNSUPPORTED_VALUE_REDACTION = "<unsupported-summary-value>"
SUMMARY_KAGEMUSHA_SHAPE_NORMALIZED_FIELD = "summary_kagemusha_shape_normalized"
SUMMARY_STATUS_NORMALIZED_FIELD = "summary_status_normalized"
SUMMARY_ERRORS_NORMALIZED_FIELD = "summary_errors_normalized"
SUMMARY_ERROR_REDACTION = "<malformed-summary-error>"
SHA256_HEX_RE = re.compile(r"^[0-9a-f]{64}$")
SIGNED_AT_UTC_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
SECRET_RE = re.compile(
    r"("
    r"authorization:|bearer\s+|private[_-]?key|token=|secret=|password=|"
    r"api[_-]?key=|x-iroha-signature"
    r")",
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


def _read_pinned_authority_file(
    path_value: str | os.PathLike[str],
    expected_sha256: str,
    *,
    label: str,
    maximum_bytes: int,
    executable: bool = False,
) -> tuple[Path | None, bytes | None, list[str]]:
    """Read one immutable-by-contract authority input without following aliases."""

    errors: list[str] = []
    path = Path(path_value)
    path_text = os.fspath(path)
    if not path.is_absolute():
        errors.append(f"{label} must be an absolute path")
        return None, None, errors
    if path_text != path_text.strip() or _contains_control_character(path_text):
        errors.append(f"{label} must be one canonical path")
        return None, None, errors
    if not isinstance(expected_sha256, str) or SHA256_HEX_RE.fullmatch(
        expected_sha256
    ) is None:
        errors.append(f"{label} SHA-256 must be 64 lowercase hex characters")
        return None, None, errors
    try:
        canonical = path.resolve(strict=True)
        path_stat = path.lstat()
    except OSError:
        errors.append(f"{label} could not be inspected")
        return None, None, errors
    if canonical != path or stat.S_ISLNK(path_stat.st_mode):
        errors.append(f"{label} must be an absolute canonical non-symlink path")
        return None, None, errors
    if not stat.S_ISREG(path_stat.st_mode):
        errors.append(f"{label} must be a regular file")
    if path_stat.st_nlink != 1:
        errors.append(f"{label} must have exactly one hard link")
    if path_stat.st_uid not in {0, os.geteuid()}:
        errors.append(f"{label} must be owned by root or the invoking user")
    if path_stat.st_mode & 0o022:
        errors.append(f"{label} must not be group- or world-writable")
    if executable and not path_stat.st_mode & stat.S_IXUSR:
        errors.append(f"{label} must be owner-executable")
    if path_stat.st_size <= 0 or path_stat.st_size > maximum_bytes:
        errors.append(f"{label} has an invalid file size")
    if errors:
        return None, None, errors

    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    chunks: list[bytes] = []
    measured = hashlib.sha256()
    try:
        descriptor = os.open(path, flags)
        try:
            open_stat = os.fstat(descriptor)
            expected_identity = (path_stat.st_dev, path_stat.st_ino)
            if (
                not stat.S_ISREG(open_stat.st_mode)
                or (open_stat.st_dev, open_stat.st_ino) != expected_identity
                or open_stat.st_nlink != 1
                or open_stat.st_size != path_stat.st_size
            ):
                errors.append(f"{label} changed while being opened")
                return None, None, errors
            size = 0
            while True:
                chunk = os.read(descriptor, min(1024 * 1024, maximum_bytes + 1 - size))
                if not chunk:
                    break
                size += len(chunk)
                if size > maximum_bytes:
                    errors.append(f"{label} exceeds its size limit")
                    return None, None, errors
                chunks.append(chunk)
                measured.update(chunk)
            final_stat = path.lstat()
            if (
                (final_stat.st_dev, final_stat.st_ino) != expected_identity
                or final_stat.st_size != size
                or final_stat.st_mtime_ns != path_stat.st_mtime_ns
                or final_stat.st_ctime_ns != path_stat.st_ctime_ns
            ):
                errors.append(f"{label} changed while being read")
                return None, None, errors
        finally:
            os.close(descriptor)
    except OSError:
        errors.append(f"{label} could not be read")
        return None, None, errors
    if measured.hexdigest() != expected_sha256:
        errors.append(f"{label} SHA-256 does not match the pinned digest")
        return None, None, errors
    return path, b"".join(chunks), errors


def _strict_json_object_bytes(payload: bytes, label: str) -> dict[str, Any]:
    def reject_duplicates(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise ValueError(f"{label} repeats JSON key {key!r}")
            result[key] = value
        return result

    try:
        decoded = payload.decode("utf-8")
        value = json.loads(
            decoded,
            object_pairs_hook=reject_duplicates,
            parse_constant=lambda token: (_ for _ in ()).throw(
                ValueError(f"{label} contains non-finite {token}")
            ),
        )
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        raise ValueError(f"{label} must be strict UTF-8 JSON: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def configure_android_evidence_authority(
    *,
    java: str | os.PathLike[str],
    java_sha256: str,
    apksigner_jar: str | os.PathLike[str],
    apksigner_jar_sha256: str,
    openssl: str | os.PathLike[str],
    openssl_sha256: str,
    attestation_trust_roots: Iterable[str | os.PathLike[str]],
    attestation_trust_root_sha256: Iterable[str],
    attestation_revocation_status: str | os.PathLike[str],
    attestation_revocation_status_sha256: str,
) -> list[str]:
    """Install the explicit, digest-pinned local authority configuration."""

    global _ANDROID_EVIDENCE_AUTHORITY
    _ANDROID_EVIDENCE_AUTHORITY = None
    errors: list[str] = []
    java_path, _, java_errors = _read_pinned_authority_file(
        java,
        java_sha256,
        label="--java",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    errors.extend(java_errors)
    apksigner_jar_path, _, apksigner_jar_errors = _read_pinned_authority_file(
        apksigner_jar,
        apksigner_jar_sha256,
        label="--apksigner-jar",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
    )
    errors.extend(apksigner_jar_errors)
    openssl_path, _, openssl_errors = _read_pinned_authority_file(
        openssl,
        openssl_sha256,
        label="--openssl",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    errors.extend(openssl_errors)

    roots = list(attestation_trust_roots)
    root_digests = list(attestation_trust_root_sha256)
    if not roots or len(roots) != len(root_digests):
        errors.append(
            "Android attestation trust-root paths and SHA-256 pins must be non-empty and aligned"
        )
    root_records: list[dict[str, Any]] = []
    for index, (root, digest) in enumerate(zip(roots, root_digests)):
        root_path, root_bytes, root_errors = _read_pinned_authority_file(
            root,
            digest,
            label=f"--android-attestation-trust-root[{index}]",
            maximum_bytes=MAX_ATTESTATION_CERTIFICATE_CHAIN_BYTES,
        )
        errors.extend(root_errors)
        if root_path is not None and root_bytes is not None:
            root_records.append(
                {"path": root_path, "sha256": digest, "bytes": root_bytes}
            )

    status_path, status_bytes, status_errors = _read_pinned_authority_file(
        attestation_revocation_status,
        attestation_revocation_status_sha256,
        label="--android-attestation-revocation-status",
        maximum_bytes=MAX_ANDROID_ATTESTATION_REVOCATION_STATUS_BYTES,
    )
    errors.extend(status_errors)
    revocation_status: dict[str, Any] | None = None
    if status_bytes is not None:
        try:
            revocation_status = _strict_json_object_bytes(
                status_bytes,
                "Android attestation revocation status",
            )
        except ValueError as error:
            errors.append(str(error))
        else:
            entries = revocation_status.get("entries")
            if set(revocation_status) != {"entries"} or not isinstance(entries, dict):
                errors.append(
                    "Android attestation revocation status must contain exactly an entries object"
                )
            else:
                for serial, record in entries.items():
                    if (
                        not isinstance(serial, str)
                        or re.fullmatch(r"(?:0|[1-9a-f][0-9a-f]*)", serial) is None
                        or not isinstance(record, dict)
                        or not isinstance(record.get("status"), str)
                        or record.get("status") == ""
                    ):
                        errors.append(
                            "Android attestation revocation status contains a malformed entry"
                        )
                        break

    if errors:
        return errors
    assert java_path is not None
    assert apksigner_jar_path is not None
    assert openssl_path is not None
    assert status_path is not None
    assert revocation_status is not None
    _ANDROID_EVIDENCE_AUTHORITY = {
        "java": {"path": java_path, "sha256": java_sha256},
        "apksigner_jar": {
            "path": apksigner_jar_path,
            "sha256": apksigner_jar_sha256,
        },
        "openssl": {"path": openssl_path, "sha256": openssl_sha256},
        "attestation_trust_roots": tuple(root_records),
        "attestation_revocation_status": {
            "path": status_path,
            "sha256": attestation_revocation_status_sha256,
            "payload": revocation_status,
        },
    }
    return []


def _configure_android_evidence_authority_from_args(
    args: argparse.Namespace,
) -> list[str]:
    """Forward one complete CLI authority request to the public configurator."""

    return configure_android_evidence_authority(
        java=args.java,
        java_sha256=args.java_sha256,
        apksigner_jar=args.apksigner_jar,
        apksigner_jar_sha256=args.apksigner_jar_sha256,
        openssl=args.openssl,
        openssl_sha256=args.openssl_sha256,
        attestation_trust_roots=args.android_attestation_trust_root or [],
        attestation_trust_root_sha256=(
            args.android_attestation_trust_root_sha256 or []
        ),
        attestation_revocation_status=args.android_attestation_revocation_status,
        attestation_revocation_status_sha256=(
            args.android_attestation_revocation_status_sha256
        ),
    )


def android_evidence_authority_projection() -> dict[str, Any] | None:
    """Return the non-secret digests bound into release summaries."""

    authority = _ANDROID_EVIDENCE_AUTHORITY
    if authority is None:
        return None
    return {
        "java_sha256": authority["java"]["sha256"],
        "apksigner_jar_sha256": authority["apksigner_jar"]["sha256"],
        "openssl_sha256": authority["openssl"]["sha256"],
        "attestation_trust_root_sha256": sorted(
            root["sha256"] for root in authority["attestation_trust_roots"]
        ),
        "attestation_revocation_status_sha256": authority[
            "attestation_revocation_status"
        ]["sha256"],
    }


def _configured_authority_tool(name: str) -> tuple[Path, str]:
    authority = _ANDROID_EVIDENCE_AUTHORITY
    if authority is None:
        raise ValueError("digest-pinned Android evidence authority tools are required")
    record = authority[name]
    return record["path"], record["sha256"]
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
    "scripts/run_kagemusha_candidate_android_lab.sh",
    "scripts/stage_kagemusha_candidate_android_artifacts.py",
    "--build-only",
    "--stage-sha256",
    "--attestation-slot",
    "--trusted-signer-public-key",
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
    "KagemushaCandidateLifecycleInstrumentedTest",
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
    "KagemushaCandidateArtifactExportInstrumentedTest",
    "kagemushaAttestationChallengeHex",
    "kagemushaStrongboxAttestation true",
    "kagemushaPhysicalDeviceAttestation true",
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND = (
    "scripts/run_kagemusha_candidate_android_lab.sh --build-only "
    '--candidate-sha256 "$CANDIDATE_SHA256" '
    '--stage-sha256 "$STAGE_SHA256" '
    '--source-commit "$SOURCE_COMMIT" '
    '--source-tree-sha256 "$SOURCE_TREE_SHA256" '
    '--generation "$GENERATION" --slot-id "$SLOT_ID"'
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND = (
    "scripts/run_kagemusha_candidate_android_lab.sh "
    '--candidate-sha256 "$CANDIDATE_SHA256" '
    '--stage-sha256 "$STAGE_SHA256" '
    '--source-commit "$SOURCE_COMMIT" '
    '--source-tree-sha256 "$SOURCE_TREE_SHA256" '
    '--generation "$GENERATION" --slot-id "$SLOT_ID" '
    '--attestation-slot "$SLOT_PATH" '
    '--trusted-signer-public-key "$TRUSTED_SIGNER_PUBLIC_KEY"'
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND = (
    "python3 -I scripts/stage_kagemusha_candidate_android_artifacts.py "
    '--adb "$ADB_BINARY" --evidence-root "$EVIDENCE_ROOT" '
    '--candidate-sha256 "$CANDIDATE_SHA256" --stage-sha256 "$STAGE_SHA256" '
    '--source-commit "$SOURCE_COMMIT" '
    '--source-tree-sha256 "$SOURCE_TREE_SHA256" && '
    "adb shell am instrument -w -r -e class "
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
    "KagemushaCandidateLifecycleInstrumentedTest "
    '-e kagemushaAttestationChallengeHex "$CHALLENGE_HEX" '
    '-e kagemushaAttestationChallengeSha256 "$CHALLENGE_SHA256" '
    "-e kagemushaAttestationCertificateChainSha256 "
    '"$ATTESTATION_CERTIFICATE_CHAIN_SHA256" '
    '-e kagemushaAppSigningCertificateSha256 "$APP_SIGNING_CERTIFICATE_SHA256" '
    "-e kagemushaStrongboxAttestation true "
    "-e kagemushaPhysicalDeviceAttestation true "
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab.test/"
    "androidx.test.runner.AndroidJUnitRunner"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND = (
    "adb shell am instrument -w -r -e class "
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab."
    "KagemushaCandidateArtifactExportInstrumentedTest "
    '-e kagemushaAttestationChallengeHex "$CHALLENGE_HEX" '
    '-e kagemushaAttestationChallengeSha256 "$CHALLENGE_SHA256" '
    "-e kagemushaAttestationCertificateChainSha256 "
    '"$ATTESTATION_CERTIFICATE_CHAIN_SHA256" '
    '-e kagemushaAppSigningCertificateSha256 "$APP_SIGNING_CERTIFICATE_SHA256" '
    "-e kagemushaStrongboxAttestation true "
    "-e kagemushaPhysicalDeviceAttestation true "
    "org.hyperledger.iroha.sdk.kagemusha.candidate.lab.test/"
    "androidx.test.runner.AndroidJUnitRunner"
)
KAGEMUSHA_ANDROID_PRODUCTION_RAW_TEST_COMMANDS: tuple[str, ...] = (
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_BUILD_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_HARNESS_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_LIFECYCLE_COMMAND,
    KAGEMUSHA_ANDROID_PRODUCTION_RAW_EXPORT_COMMAND,
)
SIGNED_EVIDENCE_SCHEMA_V1 = "iroha.android.device_lab.kagemusha.signed_evidence.v1"
SIGNED_EVIDENCE_SCHEMA_V2 = "iroha.android.device_lab.kagemusha.signed_evidence.v2"
SIGNED_EVIDENCE_SCHEMA = SIGNED_EVIDENCE_SCHEMA_V2
KAGEMUSHA_SLOT_SCHEMA_V1 = "iroha.android.device_lab.kagemusha.v1"
KAGEMUSHA_SLOT_SCHEMA_V2 = "iroha.android.device_lab.kagemusha.v2"
KAGEMUSHA_CANDIDATE_BINDING_SCHEMA_V2 = (
    "iroha.android.device_lab.kagemusha.candidate_binding.v2"
)
KAGEMUSHA_CANDIDATE_LIFECYCLE_SCHEMA_V2 = (
    "iroha.android.device_lab.kagemusha.lifecycle_transcript.v2"
)
KAGEMUSHA_CANDIDATE_CAUSAL_EVENT_FIELDS_V1: frozenset[str] = frozenset(
    {
        "sequence",
        "phase",
        "operation",
        "outcome",
        "duration_nanos",
        "input_sha256",
        "output_sha256",
        "output_size_bytes",
        "rejection_classification",
        "exception_class",
        "error_message_sha256",
    }
)
KAGEMUSHA_CANDIDATE_CAUSAL_OPERATIONS_V1: tuple[str, ...] = (
    "candidate_install",
    "build_init_request",
    "init",
    "build_append_hop_01_request",
    "append_hop_01",
    "build_append_hop_02_request",
    "append_hop_02",
    "candidate_reinstall_after_process_restart",
    "restore_init_result_after_restart",
    "restore_hop_01_result_after_restart",
    "restore_hop_02_result_after_restart",
    "validate_init_branch_after_restart",
    "validate_hop_01_change_continuity",
    "validate_hop_01_recipient_branch",
    "validate_hop_02_recipient_branch",
    "validate_sender_change_branch",
    "build_verify_first_recipient_proof_request",
    "verify_first_recipient_proof",
    "build_verify_multi_hop_recipient_proof_request",
    "verify_multi_hop_recipient_proof",
    "build_duplicate_input_request_from_observed_branch",
    "duplicate_input_rejection",
    "build_redeem_first_recipient_request",
    "redeem_first_recipient",
    "build_redeem_second_recipient_request",
    "redeem_second_recipient",
    "build_redeem_sender_change_request",
    "redeem_sender_change",
)
KAGEMUSHA_CANDIDATE_ARTIFACT_ROLES_V4: tuple[str, ...] = (
    "step_eq_params_ipa",
    "step_eq_proving_key",
    "step_eq_verifying_key",
    "step_eq_bootstrap_witness",
    "step_ep_params_ipa",
    "step_ep_proving_key",
    "step_ep_verifying_key",
    "step_ep_bootstrap_witness",
)
KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4: tuple[str, ...] = (
    "step-eq.params-ipa.krv4",
    "step-eq.proving-key.krv4",
    "step-eq.verifying-key.krv4",
    "step-eq.bootstrap-witness.krv4",
    "step-ep.params-ipa.krv4",
    "step-ep.proving-key.krv4",
    "step-ep.verifying-key.krv4",
    "step-ep.bootstrap-witness.krv4",
)
KAGEMUSHA_CANDIDATE_BINDING_FIELDS_V2: frozenset[str] = frozenset(
    {
        "schema",
        "candidate_record_path",
        "candidate_record_sha256",
        "candidate_manifest_path",
        "candidate_manifest_sha256",
        "candidate_stage_manifest_path",
        "candidate_stage_manifest_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "generation",
        "bridge_abi_version",
        "lab_native_library_path",
        "lab_native_library_sha256",
        "lab_apk_path",
        "lab_apk_sha256",
        "lab_apk_signing_cert_sha256",
        "lab_test_apk_path",
        "lab_test_apk_sha256",
        "lab_test_apk_signing_cert_sha256",
        "production_capability_observed",
        "native_accepted_candidate_record_sha256",
        "native_accepted_candidate_manifest_sha256",
        "native_accepted_source_commit",
        "native_accepted_source_tree_sha256",
        "native_accepted_source_repo_dirty",
        "native_accepted_generation",
        "native_accepted_bridge_abi_version",
        "native_accepted_inventory_sha256",
        "lifecycle_transcript_path",
        "lifecycle_transcript_sha256",
        "artifact_inventory",
    }
)
KAGEMUSHA_CANDIDATE_ARTIFACT_ENTRY_FIELDS_V2: frozenset[str] = frozenset(
    {
        "role",
        "path",
        "framed_size_bytes",
        "framed_sha256",
        "payload_size_bytes",
        "payload_sha256",
    }
)
KAGEMUSHA_CANDIDATE_LIFECYCLE_FIELDS_V2: frozenset[str] = frozenset(
    {
        "schema",
        "slot_id",
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_stage_manifest_path",
        "candidate_stage_manifest_sha256",
        "candidate_inventory_sha256",
        "source_commit",
        "source_tree_sha256",
        "source_repo_dirty",
        "generation",
        "bridge_abi_version",
        "production_capability_observed",
        "initial_atomic",
        "first_recipient_atomic",
        "second_recipient_atomic",
        "sender_change_atomic",
        "redeemed_atomic",
        "final_unspent_atomic",
        "proof_hops",
        "init_proof_verified",
        "first_spend_verified",
        "multi_hop_proof_verified",
        "independent_branch_redemption_verified",
        "duplicate_rejected",
        "restart_recovered",
        "network_requests_during_peer_transfers",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_sha256",
        "app_signing_certificate_sha256",
        "strongbox_attestation",
        "physical_device_attestation",
        "causal_events",
    }
)
D2D_PAYMENT_TRANSCRIPT_SCHEMA = "iroha.android.device_lab.kagemusha.d2d_payment.v1"
D2D_PAYMENT_PAYLOAD_SCHEMA = "kagemusha.recursive_spend.d2d.v1"
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
REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION = 21
KAGEMUSHA_RECURSIVE_SPEND_JNI_PROBE_STATES = {"recursive_spend_verified"}
KAGEMUSHA_RECURSIVE_SPEND_PROVER_STATES = {"multi_hop_proof_composed"}
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
    "kagemusha_wallet_apk_path",
    "d2d_payment_transcript_path",
    "wallet_integrity_transcript_path",
    "keymint_security_level",
    "kagemusha_recursive_spend_ffi_surface",
    "kagemusha_recursive_spend_jni_probe",
    "kagemusha_recursive_spend_prover_state",
    "candidate_binding_path",
    "candidate_record_path",
    "candidate_manifest_path",
    "candidate_stage_manifest_path",
    "candidate_source_commit",
    "candidate_generation",
    "candidate_lab_native_library_path",
    "candidate_lab_apk_path",
    "candidate_lab_test_apk_path",
    "candidate_lifecycle_transcript_path",
)
SIGNED_EVIDENCE_SLOT_ARTIFACT_PATH_FIELDS: tuple[str, ...] = (
    "attestation_certificate_chain_path",
    "kagemusha_wallet_apk_path",
    "d2d_payment_transcript_path",
    "wallet_integrity_transcript_path",
    "candidate_binding_path",
    "candidate_record_path",
    "candidate_manifest_path",
    "candidate_stage_manifest_path",
    "candidate_lab_native_library_path",
    "candidate_lab_apk_path",
    "candidate_lab_test_apk_path",
    "candidate_lifecycle_transcript_path",
)
SIGNED_EVIDENCE_SLOT_SHA256_FIELDS: tuple[str, ...] = (
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "attestation_certificate_chain_sha256",
    "kagemusha_wallet_policy_sha256",
    "kagemusha_wallet_apk_sha256",
    "d2d_payment_transcript_sha256",
    "wallet_integrity_transcript_sha256",
    "candidate_binding_sha256",
    "candidate_record_sha256",
    "candidate_manifest_sha256",
    "candidate_stage_manifest_sha256",
    "candidate_source_tree_sha256",
    "candidate_source_tree_sha256_before",
    "candidate_source_tree_sha256_after",
    "candidate_lab_native_library_sha256",
    "candidate_lab_apk_sha256",
    "candidate_lab_test_apk_sha256",
    "candidate_lab_apk_signing_certificate_sha256",
    "candidate_lab_test_apk_signing_certificate_sha256",
    "candidate_lifecycle_transcript_sha256",
    "candidate_inventory_sha256",
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
SIGNED_EVIDENCE_SLOT_FALSE_FIELDS: tuple[str, ...] = (
    "production_capability_observed",
    "candidate_source_repo_dirty",
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
        *SIGNED_EVIDENCE_SLOT_FALSE_FIELDS,
        "raw_test_commands",
        D2D_PAYMENT_TRANSCRIPTS_FIELD,
        "signed_evidence_artifact_path",
        "signed_evidence_artifact_sha256",
    }
)
SIGNED_EVIDENCE_FIELDS: frozenset[str] = frozenset(
    {
        "schema",
        *SIGNED_EVIDENCE_SLOT_STRING_FIELDS,
        *SIGNED_EVIDENCE_SLOT_SHA256_FIELDS,
        *SIGNED_EVIDENCE_SLOT_INT_FIELDS,
        *SIGNED_EVIDENCE_SLOT_TRUE_FIELDS,
        *SIGNED_EVIDENCE_SLOT_FALSE_FIELDS,
        "slot_id",
        "device_family",
        "device_model",
        "device_codename",
        "device_fingerprint",
        "os_build_id",
        "minimum_os",
        "app_package_name",
        "attestation_certificate_chain_path",
        "kagemusha_wallet_apk_path",
        "d2d_payment_transcript_path",
        "wallet_integrity_transcript_path",
        "app_signing_certificate_sha256",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_sha256",
        "kagemusha_wallet_policy_sha256",
        "kagemusha_wallet_apk_sha256",
        "d2d_payment_transcript_sha256",
        "wallet_integrity_transcript_sha256",
        "native_bridge_abi_version",
        "strongbox_attestation",
        "physical_device_attestation",
        "keymint_security_level",
        "one_use_key_rotation_passed",
        "rollback_rejection_passed",
        "kagemusha_recursive_spend_ffi_surface",
        "kagemusha_recursive_spend_jni_probe",
        "kagemusha_recursive_spend_prover_state",
        D2D_PAYMENT_TRANSCRIPTS_FIELD,
        "raw_test_commands",
        "signed_at_utc",
        "signer_key_id",
        "signer_public_key_sha256",
        "signature_algorithm",
        "signature_payload_sha256",
        "signature",
        "artifact_digests",
        *SIGNED_EVIDENCE_SLOT_FALSE_FIELDS,
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
    "kagemusha_wallet_policy_sha256",
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
    "kagemusha_wallet_policy_sha256",
    "kagemusha_wallet_apk_sha256",
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
    "kagemusha_wallet_policy_sha256",
    "kagemusha_wallet_apk_sha256",
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
    ("kagemusha_wallet_apk_path", "kagemusha_wallet_apk_sha256"),
    ("d2d_payment_transcript_path", "d2d_payment_transcript_sha256"),
    ("wallet_integrity_transcript_path", "wallet_integrity_transcript_sha256"),
    ("candidate_record_path", "candidate_record_sha256"),
    ("candidate_manifest_path", "candidate_manifest_sha256"),
    ("candidate_stage_manifest_path", "candidate_stage_manifest_sha256"),
    ("candidate_lab_native_library_path", "candidate_lab_native_library_sha256"),
    ("candidate_lab_apk_path", "candidate_lab_apk_sha256"),
    ("candidate_lab_test_apk_path", "candidate_lab_test_apk_sha256"),
    ("candidate_lifecycle_transcript_path", "candidate_lifecycle_transcript_sha256"),
    ("candidate_binding_path", "candidate_binding_sha256"),
)
KAGEMUSHA_SUMMARY_RELEASE_ARTIFACT_ROOTS: dict[str, str] = {
    "attestation_certificate_chain_path": "attestation",
    "kagemusha_wallet_apk_path": "evidence",
    "d2d_payment_transcript_path": "handoff",
    "wallet_integrity_transcript_path": "wallet",
    "candidate_record_path": "evidence",
    "candidate_manifest_path": "evidence",
    "candidate_stage_manifest_path": "<slot-root>",
    "candidate_lab_native_library_path": "evidence",
    "candidate_lab_apk_path": "evidence",
    "candidate_lab_test_apk_path": "evidence",
    "candidate_lifecycle_transcript_path": "evidence",
    "candidate_binding_path": "evidence",
}
KAGEMUSHA_SUMMARY_RELEASE_SHA256_FIELDS: tuple[str, ...] = (
    "signed_evidence_artifact_sha256",
    "signed_evidence_signer_public_key_sha256",
    "device_fingerprint_sha256",
    "app_signing_certificate_sha256",
    "attestation_challenge_sha256",
    "candidate_source_tree_sha256_before",
    "candidate_source_tree_sha256_after",
    "candidate_lab_apk_signing_certificate_sha256",
    "candidate_lab_test_apk_signing_certificate_sha256",
    "candidate_source_tree_sha256",
    "candidate_inventory_sha256",
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
        "candidate_source_commit",
        "candidate_generation",
        "production_capability_observed",
        "candidate_source_repo_dirty",
        "strongbox_attestation",
        "physical_device_attestation",
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
    source_commit = kagemusha.get("candidate_source_commit")
    generation = kagemusha.get("candidate_generation")
    if (
        not isinstance(source_commit, str)
        or re.fullmatch(r"[0-9a-f]{40}", source_commit) is None
        or source_commit == "0" * 40
        or not isinstance(generation, str)
        or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", generation) is None
        or kagemusha.get("production_capability_observed") is not False
        or kagemusha.get("candidate_source_repo_dirty") is not False
    ):
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
        if path_field == "candidate_stage_manifest_path":
            if kagemusha.get(path_field) != KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2:
                return None
        elif not _summary_release_artifact_path_under(kagemusha.get(path_field), root):
            return None
    if (
        kagemusha.get("strongbox_attestation") is not True
        or kagemusha.get("physical_device_attestation") is not True
    ):
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
    seen_paths: set[str] = set()
    seen_digests: set[str] = set()
    for transport in declared_transports:
        binding = _summary_release_d2d_transcript_binding(transcripts.get(transport))
        if binding is None:
            return False
        path, digest = binding
        if path in seen_paths:
            return False
        if digest in seen_digests:
            return False
        seen_paths.add(path)
        seen_digests.add(digest)
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


def _summary_release_d2d_payment_transport_coverage_by_family(
    reports: list[dict],
    trusted_signer_public_key_sha256: frozenset[str] | None = None,
) -> dict[str, list[str]]:
    """Return complete release D2D transport coverage by standard device family."""

    coverage: dict[str, set[str]] = {
        family: set() for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES
    }
    for report in reports:
        family = _summary_release_device_family(
            report,
            trusted_signer_public_key_sha256,
        )
        if family is None:
            continue
        coverage[family].update(
            _summary_release_d2d_payment_transports(
                report,
                trusted_signer_public_key_sha256,
            )
        )
    return {family: sorted(transports) for family, transports in coverage.items()}


def _missing_summary_release_d2d_payment_transport_pairs(
    coverage_by_family: dict[str, list[str]],
) -> list[dict[str, str]]:
    """Return required standard-family D2D transport pairs without evidence."""

    missing: list[dict[str, str]] = []
    for family in KAGEMUSHA_STANDARD_DEVICE_FAMILIES:
        covered = set(coverage_by_family.get(family, []))
        for transport in sorted(D2D_PAYMENT_TRANSPORTS):
            if transport not in covered:
                missing.append({"device_family": family, "transport": transport})
    return missing


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


def _summary_duplicate_matrix_values(
    kagemusha: dict[str, Any],
    field: str,
) -> set[str]:
    """Return canonical duplicate-binding values from a release Kagemusha report."""

    values: set[str] = set()
    value = kagemusha.get(field)
    if (
        isinstance(value, str)
        and SHA256_HEX_RE.fullmatch(value)
        and value != "0" * 64
    ):
        values.add(value)
    if field != "d2d_payment_transcript_sha256":
        return values
    transcripts = kagemusha.get(D2D_PAYMENT_TRANSCRIPTS_FIELD)
    if not isinstance(transcripts, dict):
        return values
    for entry in transcripts.values():
        if not isinstance(entry, dict):
            continue
        digest = entry.get("sha256")
        if (
            isinstance(digest, str)
            and SHA256_HEX_RE.fullmatch(digest)
            and digest != "0" * 64
        ):
            values.add(digest)
    return values


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
    except NonFiniteJsonConstantError:
        errors.append(
            f"{label} contains non-finite constant {JSON_NONFINITE_CONSTANT_REDACTION}"
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


def _require_false(data: dict[str, Any], key: str, errors: list[str]) -> None:
    if data.get(key) is not False:
        errors.append(f"slot.json {key} must be false")


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


def _require_evidence_false(data: dict[str, Any], key: str, errors: list[str]) -> None:
    if data.get(key) is not False:
        errors.append(f"signed evidence artifact {key} must be false")


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
    attestation_certificate_count: int | None = None,
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
    if challenge is not None and len(challenge) != 32:
        errors.append(
            "attestation/harness-result.json challenge_hex must encode exactly 32 bytes"
        )
    if challenge is not None:
        try:
            derived_challenge = derive_kagemusha_strongbox_challenge_v1(metadata)
        except ValueError as error:
            errors.append(f"candidate StrongBox challenge inputs are invalid: {error}")
        else:
            if challenge != derived_challenge:
                errors.append(
                    "attestation/harness-result.json challenge_hex must equal the "
                    "candidate-stage StrongBox challenge"
                )
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
    elif attestation_certificate_count is not None:
        if chain_length != attestation_certificate_count:
            errors.append(
                "attestation/harness-result.json chain_length must match "
                "attestation certificate-chain certificate count"
            )
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
    seen_digests: dict[str, str] = {}
    if primary_relative is not None and primary_transport is not None:
        seen_paths[primary_relative] = primary_transport
    if primary_digest is not None and primary_transport is not None:
        seen_digests[primary_digest] = primary_transport
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
        previous_digest_transport = seen_digests.get(validated["sha256"])
        if (
            previous_digest_transport is not None
            and previous_digest_transport != transport
        ):
            errors.append(
                f"slot.json {D2D_PAYMENT_TRANSCRIPTS_FIELD} must not reuse "
                "sha256 digests for multiple transports"
            )
            continue
        seen_digests[validated["sha256"]] = transport
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


class _StrictDerReader:
    """Small strict DER reader for the Android KeyDescription policy fields."""

    def __init__(self, payload: bytes):
        self.payload = payload
        self.offset = 0

    def remaining(self) -> bool:
        return self.offset < len(self.payload)

    def read(self) -> tuple[int, bool, int, bytes, bytes]:
        start = self.offset
        if self.offset >= len(self.payload):
            raise ValueError("DER value is truncated")
        first = self.payload[self.offset]
        self.offset += 1
        tag_class = first >> 6
        constructed = bool(first & 0x20)
        tag = first & 0x1F
        if tag == 0x1F:
            tag = 0
            first_tag_octet = True
            while True:
                if self.offset >= len(self.payload):
                    raise ValueError("DER high tag number is truncated")
                octet = self.payload[self.offset]
                self.offset += 1
                if first_tag_octet and octet == 0x80:
                    raise ValueError("DER high tag number is non-minimal")
                first_tag_octet = False
                if tag > (1 << 31):
                    raise ValueError("DER tag number is too large")
                tag = (tag << 7) | (octet & 0x7F)
                if not octet & 0x80:
                    break
            if tag < 31:
                raise ValueError("DER high tag number is non-minimal")
        if self.offset >= len(self.payload):
            raise ValueError("DER length is truncated")
        first_length = self.payload[self.offset]
        self.offset += 1
        if first_length < 0x80:
            length = first_length
        else:
            octets = first_length & 0x7F
            if octets == 0 or octets > 4 or self.offset + octets > len(self.payload):
                raise ValueError("DER length is invalid")
            encoded = self.payload[self.offset : self.offset + octets]
            self.offset += octets
            if encoded[0] == 0:
                raise ValueError("DER length is non-minimal")
            length = int.from_bytes(encoded, "big")
            if length < 0x80:
                raise ValueError("DER length is non-minimal")
        end = self.offset + length
        if end > len(self.payload):
            raise ValueError("DER value is truncated")
        value = self.payload[self.offset : end]
        self.offset = end
        return tag_class, constructed, tag, value, self.payload[start:end]

    def expect(
        self,
        tag_class: int,
        constructed: bool,
        tag: int,
        label: str,
    ) -> bytes:
        actual_class, actual_constructed, actual_tag, value, _ = self.read()
        if (actual_class, actual_constructed, actual_tag) != (
            tag_class,
            constructed,
            tag,
        ):
            raise ValueError(f"{label} has an unexpected DER tag")
        return value

    def finish(self, label: str) -> None:
        if self.remaining():
            raise ValueError(f"{label} contains trailing DER data")


def _der_unsigned_integer(value: bytes, label: str) -> int:
    if not value or value[0] & 0x80:
        raise ValueError(f"{label} must be a non-negative DER integer")
    if len(value) > 1 and value[0] == 0 and not value[1] & 0x80:
        raise ValueError(f"{label} DER integer is non-minimal")
    return int.from_bytes(value, "big")


def _der_boolean(value: bytes, label: str) -> bool:
    if value not in (b"\x00", b"\xff"):
        raise ValueError(f"{label} must be one canonical DER boolean")
    return value == b"\xff"


def _split_der_certificate_chain(payload: bytes) -> list[bytes]:
    reader = _StrictDerReader(payload)
    certificates: list[bytes] = []
    while reader.remaining():
        tag_class, constructed, tag, _, encoded = reader.read()
        if (tag_class, constructed, tag) != (0, True, 16):
            raise ValueError("DER attestation chain must contain only X.509 sequences")
        certificates.append(encoded)
    return certificates


def _decode_attestation_certificate_chain(relative: str, payload: bytes) -> list[bytes]:
    suffix = PurePosixPath(relative).suffix.lower()
    if suffix == ".der":
        certificates = _split_der_certificate_chain(payload)
    elif suffix == ".pem":
        pattern = re.compile(
            rb"-----BEGIN CERTIFICATE-----\r?\n([A-Za-z0-9+/=\r\n]+)"
            rb"-----END CERTIFICATE-----"
        )
        certificates = []
        position = 0
        for match in pattern.finditer(payload):
            if payload[position : match.start()].strip():
                raise ValueError("PEM attestation chain contains non-certificate data")
            encoded = re.sub(rb"\s+", b"", match.group(1))
            try:
                certificate = base64.b64decode(encoded, validate=True)
            except ValueError as error:
                raise ValueError("PEM attestation chain contains invalid base64") from error
            parsed = _split_der_certificate_chain(certificate)
            if len(parsed) != 1 or parsed[0] != certificate:
                raise ValueError("PEM attestation chain contains invalid certificate DER")
            certificates.append(certificate)
            position = match.end()
        if payload[position:].strip():
            raise ValueError("PEM attestation chain contains trailing non-certificate data")
    else:
        raise ValueError("attestation chain suffix is unsupported")
    if len(certificates) < 2:
        raise ValueError("attestation certificate chain must contain at least two certificates")
    if len(certificates) > 8:
        raise ValueError("attestation certificate chain contains too many certificates")
    digests = [hashlib.sha256(certificate).digest() for certificate in certificates]
    if len(set(digests)) != len(digests):
        raise ValueError("attestation certificate chain repeats a certificate")
    return certificates


def _x509_certificate_serial_and_attestation_extension(
    certificate: bytes,
) -> tuple[str, bytes]:
    certificate_reader = _StrictDerReader(certificate)
    certificate_sequence = certificate_reader.expect(0, True, 16, "X.509 certificate")
    certificate_reader.finish("X.509 certificate")
    outer = _StrictDerReader(certificate_sequence)
    _, tbs_constructed, tbs_tag, tbs, _ = outer.read()
    if not tbs_constructed or tbs_tag != 16:
        raise ValueError("X.509 TBSCertificate is malformed")
    outer.expect(0, True, 16, "X.509 signatureAlgorithm")
    outer.expect(0, False, 3, "X.509 signatureValue")
    outer.finish("X.509 certificate")

    reader = _StrictDerReader(tbs)
    first_class, first_constructed, first_tag, first_value, _ = reader.read()
    if (first_class, first_constructed, first_tag) == (2, True, 0):
        version_reader = _StrictDerReader(first_value)
        _der_unsigned_integer(
            version_reader.expect(0, False, 2, "X.509 version"),
            "X.509 version",
        )
        version_reader.finish("X.509 version")
        serial_value = reader.expect(0, False, 2, "X.509 serialNumber")
    elif (first_class, first_constructed, first_tag) == (0, False, 2):
        serial_value = first_value
    else:
        raise ValueError("X.509 TBSCertificate serialNumber is malformed")
    serial = _der_unsigned_integer(serial_value, "X.509 serialNumber")
    if serial == 0:
        raise ValueError("X.509 serialNumber must be positive")
    for label in (
        "X.509 signature",
        "X.509 issuer",
        "X.509 validity",
        "X.509 subject",
        "X.509 subjectPublicKeyInfo",
    ):
        reader.expect(0, True, 16, label)

    extension_payload: bytes | None = None
    while reader.remaining():
        tag_class, constructed, tag, value, _ = reader.read()
        if (tag_class, constructed, tag) != (2, True, 3):
            if tag_class == 2 and tag in {1, 2}:
                continue
            raise ValueError("X.509 TBSCertificate contains an unexpected trailing field")
        if extension_payload is not None:
            raise ValueError("X.509 TBSCertificate repeats extensions")
        extension_payload = value
    if extension_payload is None:
        raise ValueError("X.509 certificate has no extensions")

    wrapper = _StrictDerReader(extension_payload)
    extension_sequence = wrapper.expect(0, True, 16, "X.509 extensions")
    wrapper.finish("X.509 extensions")
    extensions = _StrictDerReader(extension_sequence)
    attestation_extension: bytes | None = None
    oid_value = ANDROID_KEY_ATTESTATION_EXTENSION_OID_DER[2:]
    while extensions.remaining():
        encoded_extension = extensions.expect(0, True, 16, "X.509 extension")
        extension = _StrictDerReader(encoded_extension)
        oid = extension.expect(0, False, 6, "X.509 extension OID")
        if extension.remaining():
            next_class, next_constructed, next_tag, next_value, _ = extension.read()
            if (next_class, next_constructed, next_tag) == (0, False, 1):
                _der_boolean(next_value, "X.509 extension critical")
                value = extension.expect(0, False, 4, "X.509 extension value")
            elif (next_class, next_constructed, next_tag) == (0, False, 4):
                value = next_value
            else:
                raise ValueError("X.509 extension value is malformed")
        else:
            raise ValueError("X.509 extension has no value")
        extension.finish("X.509 extension")
        if oid == oid_value:
            if attestation_extension is not None:
                raise ValueError("leaf repeats the Android key-attestation extension")
            attestation_extension = value
    if attestation_extension is None:
        raise ValueError(
            f"leaf certificate is missing Android extension {ANDROID_KEY_ATTESTATION_EXTENSION_OID}"
        )
    return format(serial, "x"), attestation_extension


def _x509_certificate_serial(certificate: bytes) -> str:
    certificate_reader = _StrictDerReader(certificate)
    certificate_sequence = certificate_reader.expect(0, True, 16, "X.509 certificate")
    certificate_reader.finish("X.509 certificate")
    outer = _StrictDerReader(certificate_sequence)
    tbs = outer.expect(0, True, 16, "X.509 TBSCertificate")
    outer.expect(0, True, 16, "X.509 signatureAlgorithm")
    outer.expect(0, False, 3, "X.509 signatureValue")
    outer.finish("X.509 certificate")
    reader = _StrictDerReader(tbs)
    first_class, first_constructed, first_tag, first_value, _ = reader.read()
    if (first_class, first_constructed, first_tag) == (2, True, 0):
        version = _StrictDerReader(first_value)
        _der_unsigned_integer(
            version.expect(0, False, 2, "X.509 version"), "X.509 version"
        )
        version.finish("X.509 version")
        serial_value = reader.expect(0, False, 2, "X.509 serialNumber")
    elif (first_class, first_constructed, first_tag) == (0, False, 2):
        serial_value = first_value
    else:
        raise ValueError("X.509 TBSCertificate serialNumber is malformed")
    serial = _der_unsigned_integer(serial_value, "X.509 serialNumber")
    if serial == 0:
        raise ValueError("X.509 serialNumber must be positive")
    return format(serial, "x")


def _parse_attestation_application_id(value: bytes) -> tuple[set[str], set[bytes]]:
    explicit = _StrictDerReader(value)
    encoded = explicit.expect(0, False, 4, "attestationApplicationId OCTET STRING")
    explicit.finish("attestationApplicationId")
    wrapper = _StrictDerReader(encoded)
    sequence = wrapper.expect(0, True, 16, "attestationApplicationId")
    wrapper.finish("attestationApplicationId")
    reader = _StrictDerReader(sequence)
    packages_bytes = reader.expect(0, True, 17, "attestation packageInfos")
    digests_bytes = reader.expect(0, True, 17, "attestation signatureDigests")
    reader.finish("attestationApplicationId")

    packages: set[str] = set()
    package_reader = _StrictDerReader(packages_bytes)
    while package_reader.remaining():
        package_sequence = package_reader.expect(0, True, 16, "attestation packageInfo")
        package = _StrictDerReader(package_sequence)
        name_bytes = package.expect(0, False, 4, "attestation packageName")
        _der_unsigned_integer(
            package.expect(0, False, 2, "attestation packageVersion"),
            "attestation packageVersion",
        )
        package.finish("attestation packageInfo")
        try:
            name = name_bytes.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ValueError("attestation packageName must be UTF-8") from error
        if not name or name in packages:
            raise ValueError("attestationApplicationId repeats or empties a package name")
        packages.add(name)

    digests: set[bytes] = set()
    digest_reader = _StrictDerReader(digests_bytes)
    while digest_reader.remaining():
        digest = digest_reader.expect(0, False, 4, "attestation signatureDigest")
        if len(digest) != 32 or digest in digests:
            raise ValueError("attestationApplicationId has an invalid signing digest")
        digests.add(digest)
    if not packages or not digests:
        raise ValueError("attestationApplicationId must bind a package and signing digest")
    return packages, digests


def _parse_android_root_of_trust(value: bytes) -> None:
    explicit = _StrictDerReader(value)
    sequence = explicit.expect(0, True, 16, "rootOfTrust")
    explicit.finish("rootOfTrust")
    reader = _StrictDerReader(sequence)
    verified_boot_key = reader.expect(0, False, 4, "verifiedBootKey")
    locked = _der_boolean(reader.expect(0, False, 1, "deviceLocked"), "deviceLocked")
    state = _der_unsigned_integer(
        reader.expect(0, False, 10, "verifiedBootState"),
        "verifiedBootState",
    )
    verified_boot_hash = (
        reader.expect(0, False, 4, "verifiedBootHash") if reader.remaining() else None
    )
    reader.finish("rootOfTrust")
    if not verified_boot_key:
        raise ValueError("verifiedBootKey must be non-empty")
    if not locked:
        raise ValueError("Android attestation requires deviceLocked=true")
    if state != ANDROID_VERIFIED_BOOT_STATE_VERIFIED:
        raise ValueError("Android attestation requires verifiedBootState=Verified")
    if verified_boot_hash is None or len(verified_boot_hash) != 32:
        raise ValueError("Android StrongBox attestation requires a SHA-256 verifiedBootHash")


def _parse_android_authorization_list(
    value: bytes,
    *,
    hardware: bool,
) -> tuple[list[tuple[set[str], set[bytes]]], int]:
    reader = _StrictDerReader(value)
    applications: list[tuple[set[str], set[bytes]]] = []
    roots = 0
    seen_tags: set[int] = set()
    while reader.remaining():
        tag_class, _, tag, entry, _ = reader.read()
        if tag_class != 2:
            raise ValueError("Android authorization entry must be context-specific")
        if tag in seen_tags:
            raise ValueError(f"Android authorization list repeats tag {tag}")
        seen_tags.add(tag)
        if tag == ANDROID_TAG_ALL_APPLICATIONS:
            raise ValueError("Android attestation must not authorize all applications")
        if tag == ANDROID_TAG_ATTESTATION_APPLICATION_ID:
            applications.append(_parse_attestation_application_id(entry))
        elif tag == ANDROID_TAG_ROOT_OF_TRUST:
            if not hardware:
                raise ValueError("rootOfTrust must be hardware-enforced")
            _parse_android_root_of_trust(entry)
            roots += 1
    return applications, roots


def _parse_android_key_description(
    extension: bytes,
    *,
    expected_challenge: bytes,
    expected_package: str,
    expected_signing_digest: bytes,
) -> None:
    wrapper = _StrictDerReader(extension)
    sequence = wrapper.expect(0, True, 16, "Android KeyDescription")
    wrapper.finish("Android KeyDescription")
    reader = _StrictDerReader(sequence)
    attestation_version = _der_unsigned_integer(
        reader.expect(0, False, 2, "attestationVersion"), "attestationVersion"
    )
    attestation_level = _der_unsigned_integer(
        reader.expect(0, False, 10, "attestationSecurityLevel"),
        "attestationSecurityLevel",
    )
    keymint_version = _der_unsigned_integer(
        reader.expect(0, False, 2, "keyMintVersion"), "keyMintVersion"
    )
    keymint_level = _der_unsigned_integer(
        reader.expect(0, False, 10, "keyMintSecurityLevel"),
        "keyMintSecurityLevel",
    )
    challenge = reader.expect(0, False, 4, "attestationChallenge")
    reader.expect(0, False, 4, "uniqueId")
    software = reader.expect(0, True, 16, "softwareEnforced")
    hardware = reader.expect(0, True, 16, "hardwareEnforced")
    reader.finish("Android KeyDescription")
    if attestation_version <= 0 or keymint_version <= 0:
        raise ValueError("Android attestation and KeyMint versions must be positive")
    if (
        attestation_level != ANDROID_SECURITY_LEVEL_STRONGBOX
        or keymint_level != ANDROID_SECURITY_LEVEL_STRONGBOX
    ):
        raise ValueError(
            "Android attestationSecurityLevel and keyMintSecurityLevel must both be StrongBox(2)"
        )
    if len(challenge) != 32 or challenge != expected_challenge:
        raise ValueError("leaf Android attestation challenge is not the exact candidate challenge")

    app_ids: list[tuple[set[str], set[bytes]]] = []
    root_count = 0
    parsed_apps, parsed_roots = _parse_android_authorization_list(
        software, hardware=False
    )
    app_ids.extend(parsed_apps)
    root_count += parsed_roots
    parsed_apps, parsed_roots = _parse_android_authorization_list(
        hardware, hardware=True
    )
    app_ids.extend(parsed_apps)
    root_count += parsed_roots
    if len(app_ids) != 1:
        raise ValueError("Android attestation must contain exactly one attestationApplicationId")
    packages, digests = app_ids[0]
    if packages != {expected_package}:
        raise ValueError("attestationApplicationId does not bind exactly the wallet package")
    if digests != {expected_signing_digest}:
        raise ValueError(
            "attestationApplicationId does not bind exactly the production wallet signing digest"
        )
    if root_count != 1:
        raise ValueError("Android attestation must contain exactly one hardware rootOfTrust")


def _certificate_pem(certificate: bytes) -> bytes:
    encoded = base64.b64encode(certificate)
    lines = [encoded[index : index + 64] for index in range(0, len(encoded), 64)]
    return b"-----BEGIN CERTIFICATE-----\n" + b"\n".join(lines) + (
        b"\n-----END CERTIFICATE-----\n"
    )


def _run_pinned_openssl(arguments: list[str]) -> subprocess.CompletedProcess[bytes]:
    openssl, openssl_sha256 = _configured_authority_tool("openssl")
    checked, _, errors = _read_pinned_authority_file(
        openssl,
        openssl_sha256,
        label="configured openssl",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    if checked is None or errors:
        raise ValueError("configured openssl no longer matches its authority pin")
    completed = subprocess.run(
        [os.fspath(openssl), *arguments],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=_openssl_child_env(),
        check=False,
    )
    checked, _, errors = _read_pinned_authority_file(
        openssl,
        openssl_sha256,
        label="configured openssl",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    if checked is None or errors:
        raise ValueError("configured openssl changed during verification")
    return completed


def _decode_single_trust_root(path: Path, payload: bytes) -> bytes:
    if path.suffix.lower() == ".der":
        certificates = _split_der_certificate_chain(payload)
    elif path.suffix.lower() == ".pem":
        pattern = re.compile(
            rb"^\s*-----BEGIN CERTIFICATE-----\r?\n([A-Za-z0-9+/=\r\n]+)"
            rb"-----END CERTIFICATE-----\s*$"
        )
        match = pattern.fullmatch(payload)
        if match is None:
            raise ValueError("Android attestation trust root PEM is malformed")
        try:
            certificate = base64.b64decode(
                re.sub(rb"\s+", b"", match.group(1)), validate=True
            )
        except ValueError as error:
            raise ValueError("Android attestation trust root PEM is malformed") from error
        certificates = _split_der_certificate_chain(certificate)
    else:
        raise ValueError("Android attestation trust root must end in .der or .pem")
    if len(certificates) != 1:
        raise ValueError("each Android attestation trust root must contain one certificate")
    return certificates[0]


def _validate_android_attestation_certificate_chain(
    relative: str,
    payload: bytes,
    metadata: Mapping[str, Any],
    errors: list[str],
) -> int | None:
    """Cryptographically validate and independently project Android attestation."""

    authority = _ANDROID_EVIDENCE_AUTHORITY
    if authority is None:
        errors.append("digest-pinned Android attestation authority inputs are required")
        return None
    try:
        certificates = _decode_attestation_certificate_chain(relative, payload)
        roots = [
            _decode_single_trust_root(record["path"], record["bytes"])
            for record in authority["attestation_trust_roots"]
        ]
        root_by_digest = {
            hashlib.sha256(root).hexdigest(): root for root in roots
        }
        if len(root_by_digest) != len(roots):
            raise ValueError("Android attestation trust roots contain duplicates")
        if hashlib.sha256(certificates[-1]).hexdigest() not in root_by_digest:
            raise ValueError("attestation chain is not terminated by an explicit trusted root")

        expected_challenge = derive_kagemusha_strongbox_challenge_v1(metadata)
        expected_package = metadata.get("app_package_name")
        expected_signer = metadata.get("app_signing_certificate_sha256")
        if expected_package != KAGEMUSHA_WALLET_PACKAGE_NAME:
            raise ValueError("slot app_package_name is not the production wallet package")
        if not isinstance(expected_signer, str) or SHA256_HEX_RE.fullmatch(
            expected_signer
        ) is None:
            raise ValueError("slot wallet signing-certificate digest is invalid")
        serials: list[str] = []
        serial, extension = _x509_certificate_serial_and_attestation_extension(
            certificates[0]
        )
        serials.append(serial)
        _parse_android_key_description(
            extension,
            expected_challenge=expected_challenge,
            expected_package=expected_package,
            expected_signing_digest=bytes.fromhex(expected_signer),
        )
        for certificate in certificates[1:]:
            serials.append(_x509_certificate_serial(certificate))

        entries = authority["attestation_revocation_status"]["payload"]["entries"]
        for serial_number in serials[:-1]:
            if serial_number in entries:
                raise ValueError(
                    "Android attestation certificate serial is present in the authenticated revocation status"
                )

        with tempfile.TemporaryDirectory(prefix="iroha-android-attestation-") as temp:
            temp_path = Path(temp)
            os.chmod(temp_path, 0o700)
            paths: list[Path] = []
            for index, certificate in enumerate(certificates):
                certificate_path = temp_path / f"chain-{index}.pem"
                certificate_path.write_bytes(_certificate_pem(certificate))
                certificate_path.chmod(0o600)
                paths.append(certificate_path)
            roots_path = temp_path / "trusted-roots.pem"
            roots_path.write_bytes(b"".join(_certificate_pem(root) for root in roots))
            roots_path.chmod(0o600)
            intermediates_path = temp_path / "intermediates.pem"
            intermediates_path.write_bytes(
                b"".join(_certificate_pem(cert) for cert in certificates[1:-1])
            )
            intermediates_path.chmod(0o600)

            for index in range(len(paths) - 1):
                completed = _run_pinned_openssl(
                    [
                        "verify",
                        "-x509_strict",
                        "-purpose",
                        "any",
                        "-partial_chain",
                        "-CAfile",
                        os.fspath(paths[index + 1]),
                        os.fspath(paths[index]),
                    ]
                )
                if completed.returncode != 0:
                    raise ValueError(
                        f"attestation certificate {index} signature/path verification failed"
                    )
            full_arguments = [
                "verify",
                "-x509_strict",
                "-purpose",
                "any",
                "-CAfile",
                os.fspath(roots_path),
            ]
            if len(certificates) > 2:
                full_arguments.extend(
                    ["-untrusted", os.fspath(intermediates_path)]
                )
            full_arguments.append(os.fspath(paths[0]))
            completed = _run_pinned_openssl(full_arguments)
            if completed.returncode != 0:
                raise ValueError("Android attestation PKIX path validation failed")
    except (OSError, ValueError) as error:
        errors.append(f"Android StrongBox certificate-chain validation failed: {error}")
        return None
    return len(certificates)


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
    try:
        openssl, openssl_sha256 = _configured_authority_tool("openssl")
    except ValueError:
        errors.append(
            "digest-pinned openssl is required to verify Kagemusha evidence artifacts"
        )
        return None
    checked, _, tool_errors = _read_pinned_authority_file(
        openssl,
        openssl_sha256,
        label="configured openssl",
        maximum_bytes=MAX_AUTHORITY_TOOL_BYTES,
        executable=True,
    )
    if checked is None or tool_errors:
        errors.append("configured openssl no longer matches its authority pin")
        return None
    return os.fspath(openssl)


def _openssl_child_env() -> dict[str, str]:
    """Return an OpenSSL child environment without operator config overrides."""

    return {
        "HOME": "/var/empty",
        "LANG": "C",
        "LC_ALL": "C",
        "PATH": "/usr/bin",
    }


def _openssl_public_key_der(
    public_key_path: Path,
    *,
    errors: list[str],
    label: str,
) -> bytes | None:
    if not _validate_public_key_path_shape(public_key_path, errors=errors, label=label):
        return None
    public_key_bytes = _read_bounded_public_key_bytes(
        public_key_path,
        errors=errors,
        label=label,
    )
    if public_key_bytes is None:
        return None
    if any(marker in public_key_bytes for marker in PRIVATE_KEY_PEM_MARKERS):
        errors.append(f"{label} must contain public key material, not a private key")
        return None
    try:
        completed = _run_pinned_openssl(
            [
                "pkey",
                "-pubin",
                "-in",
                str(public_key_path),
                "-pubout",
                "-outform",
                "DER",
            ]
        )
    except (OSError, ValueError):
        errors.append(f"{label} OpenSSL public key command could not be run")
        return None
    if completed.returncode != 0:
        errors.append(f"{label} must be a valid OpenSSL public key")
        return None
    return completed.stdout


def _read_bounded_public_key_bytes(
    public_key_path: Path,
    *,
    errors: list[str],
    label: str,
) -> bytes | None:
    """Read public-key material with a race-aware byte cap before marker scans."""

    try:
        expected_stat = public_key_path.lstat()
    except OSError:
        errors.append(f"{label} file could not be read")
        return None
    expected_identity = (expected_stat.st_dev, expected_stat.st_ino)
    chunks: list[bytes] = []
    size = 0
    try:
        with public_key_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            path_stat = public_key_path.lstat()
            if (
                stat.S_ISLNK(path_stat.st_mode)
                or (path_stat.st_dev, path_stat.st_ino) != expected_identity
                or (open_stat.st_dev, open_stat.st_ino) != expected_identity
            ):
                errors.append(f"{label} changed while being read")
                return None
            if open_stat.st_size > MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES:
                errors.append(
                    f"{label} must be no more than "
                    f"{MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES} bytes"
                )
                return None
            while chunk := handle.read(8192):
                chunks.append(chunk)
                size += len(chunk)
                if size > MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES:
                    errors.append(
                        f"{label} must be no more than "
                        f"{MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES} bytes"
                    )
                    return None
    except OSError:
        errors.append(f"{label} file could not be read")
        return None
    return b"".join(chunks)


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
        public_key_stat = public_key_path.stat()
    except OSError:
        errors.append(f"{label} hardlink metadata could not be read")
        return False
    if public_key_stat.st_nlink > 1:
        errors.append(f"{label} must not be hardlinked")
        return False
    if public_key_stat.st_size > MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES:
        errors.append(
            f"{label} must be no more than "
            f"{MAX_ANDROID_DEVICE_LAB_SIGNING_KEY_BYTES} bytes"
        )
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
                completed = _run_pinned_openssl(
                    [
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
                    ]
                )
            except (OSError, ValueError):
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
    for root in (*EXPECTED_DIRS, "handoff", "wallet", "scenario"):
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
        except NonFiniteJsonConstantError:
            errors.append(
                f"telemetry/status.ndjson line {line_no} contains non-finite constant "
                f"{JSON_NONFINITE_CONSTANT_REDACTION}"
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


def _candidate_binding_string(
    binding: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = binding.get(key)
    if (
        not isinstance(value, str)
        or not value
        or value != value.strip()
        or _contains_control_character(value)
        or SECRET_RE.search(value)
    ):
        errors.append(f"candidate binding {key} must be canonical non-empty text")
        return None
    return value


def _candidate_binding_sha256(
    binding: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = binding.get(key)
    if (
        not isinstance(value, str)
        or SHA256_HEX_RE.fullmatch(value) is None
        or value == "0" * 64
    ):
        errors.append(f"candidate binding {key} must be non-zero lowercase sha256 hex")
        return None
    return value


def _candidate_binding_path(
    binding: dict[str, Any], key: str, errors: list[str]
) -> str | None:
    value = _candidate_binding_string(binding, key, errors)
    if value is None:
        return None
    relative = _normalise_safe_relative_path(value, errors, f"candidate binding {key}")
    if relative is None:
        return None
    if not _safe_relative_path_is_child_of(relative, "evidence"):
        errors.append(f"candidate binding {key} must stay under evidence/")
        return None
    return relative


def _kagemusha_krv4_size_exceeds_bound(size_bytes: int) -> bool:
    """Return whether one framed file or streamed payload exceeds the V4 cap."""

    return size_bytes > MAX_KAGEMUSHA_KRV4_ARTIFACT_BYTES


def _candidate_artifact_measurement(
    slot_path: Path, relative: str, errors: list[str]
) -> dict[str, int | str] | None:
    """Measure one KRV4 frame without trusting metadata-supplied offsets."""

    artifact_path, artifact_stat, path_errors = (
        _validate_signed_evidence_artifact_for_digest(slot_path, relative)
    )
    if path_errors:
        errors.extend(path_errors)
        return None
    assert artifact_path is not None and artifact_stat is not None
    if _kagemusha_krv4_size_exceeds_bound(artifact_stat.st_size):
        errors.append(f"candidate artifact {_display_path(relative)} exceeds the KRV4 bound")
        return None
    framed = hashlib.sha256()
    payload = hashlib.sha256()
    payload_size = 0
    expected_identity = (artifact_stat.st_dev, artifact_stat.st_ino)
    try:
        with artifact_path.open("rb") as handle:
            open_stat = os.fstat(handle.fileno())
            if (open_stat.st_dev, open_stat.st_ino) != expected_identity:
                errors.append(
                    f"candidate artifact {_display_path(relative)} changed while being opened"
                )
                return None
            prefix = handle.read(12)
            if len(prefix) != 12 or prefix[:8] != b"KRV4KEY\0":
                errors.append(
                    f"candidate artifact {_display_path(relative)} has invalid KRV4 framing"
                )
                return None
            header_len = int.from_bytes(prefix[8:12], "little")
            if header_len == 0 or header_len > MAX_KAGEMUSHA_KRV4_HEADER_BYTES:
                errors.append(
                    f"candidate artifact {_display_path(relative)} has invalid KRV4 header length"
                )
                return None
            header = handle.read(header_len)
            if len(header) != header_len:
                errors.append(
                    f"candidate artifact {_display_path(relative)} has a truncated KRV4 header"
                )
                return None
            framed.update(prefix)
            framed.update(header)
            for chunk in iter(lambda: handle.read(1024 * 1024), b""):
                payload_size += len(chunk)
                if _kagemusha_krv4_size_exceeds_bound(payload_size):
                    errors.append(
                        f"candidate artifact {_display_path(relative)} exceeds the KRV4 payload bound"
                    )
                    return None
                framed.update(chunk)
                payload.update(chunk)
            if payload_size == 0:
                errors.append(
                    f"candidate artifact {_display_path(relative)} has an empty KRV4 payload"
                )
                return None
            final_stat = artifact_path.lstat()
            if (final_stat.st_dev, final_stat.st_ino) != expected_identity:
                errors.append(
                    f"candidate artifact {_display_path(relative)} changed while being read"
                )
                return None
    except OSError:
        errors.append(f"candidate artifact {_display_path(relative)} could not be read")
        return None
    return {
        "framed_size_bytes": int(artifact_stat.st_size),
        "framed_sha256": framed.hexdigest(),
        "payload_size_bytes": payload_size,
        "payload_sha256": payload.hexdigest(),
    }


def _candidate_inventory_sha256(inventory: list[dict[str, Any]]) -> str:
    digest = hashlib.sha256()
    for entry in inventory:
        digest.update(str(entry["role"]).encode("utf-8"))
        digest.update(b"\0")
        digest.update(str(entry["framed_size_bytes"]).encode("ascii"))
        digest.update(b"\0")
        digest.update(str(entry["framed_sha256"]).encode("ascii"))
        digest.update(b"\0")
        digest.update(str(entry["payload_size_bytes"]).encode("ascii"))
        digest.update(b"\0")
        digest.update(str(entry["payload_sha256"]).encode("ascii"))
        digest.update(b"\n")
    return digest.hexdigest()


def _validate_candidate_causal_events_v1(value: Any, errors: list[str]) -> None:
    if not isinstance(value, list) or len(value) != len(
        KAGEMUSHA_CANDIDATE_CAUSAL_OPERATIONS_V1
    ):
        errors.append("candidate lifecycle causal_events must contain exactly 28 events")
        return
    null_output_operations = {
        "candidate_install",
        "candidate_reinstall_after_process_restart",
        "restore_init_result_after_restart",
        "restore_hop_01_result_after_restart",
        "restore_hop_02_result_after_restart",
    }
    expected_input_counts = (
        0, 5, 1, 8, 2, 8, 2, 0, 1, 1, 1, 4, 4, 4, 4, 4,
        3, 1, 3, 1, 7, 3, 8, 1, 8, 1, 8, 1,
    )
    for sequence, (event, operation, input_count) in enumerate(
        zip(value, KAGEMUSHA_CANDIDATE_CAUSAL_OPERATIONS_V1, expected_input_counts)
    ):
        label = f"candidate lifecycle causal_events[{sequence}]"
        if not isinstance(event, dict):
            errors.append(f"{label} must be an object")
            continue
        if set(event) != KAGEMUSHA_CANDIDATE_CAUSAL_EVENT_FIELDS_V1:
            errors.append(f"{label} must have the exact V1 fields")
        if event.get("sequence") != sequence or isinstance(event.get("sequence"), bool):
            errors.append(f"{label} sequence must be {sequence}")
        expected_phase = "phase_1" if sequence < 7 else "phase_2"
        if event.get("phase") != expected_phase:
            errors.append(f"{label} phase must be {expected_phase}")
        if event.get("operation") != operation:
            errors.append(f"{label} operation must be {operation}")
        duration = event.get("duration_nanos")
        if isinstance(duration, bool) or not isinstance(duration, int) or duration <= 0:
            errors.append(f"{label} duration_nanos must be a positive integer")
        input_digests = event.get("input_sha256")
        if not isinstance(input_digests, list) or len(input_digests) != input_count:
            errors.append(f"{label} input_sha256 must contain exactly {input_count} digests")
        else:
            for digest in input_digests:
                if (
                    not isinstance(digest, str)
                    or SHA256_HEX_RE.fullmatch(digest) is None
                    or digest == "0" * 64
                ):
                    errors.append(f"{label} input_sha256 contains an invalid digest")
                    break
        output_digest = event.get("output_sha256")
        output_size = event.get("output_size_bytes")
        if operation == "duplicate_input_rejection":
            if event.get("outcome") != "rejected":
                errors.append(f"{label} outcome must be rejected")
            if output_digest is not None or output_size != 0:
                errors.append(f"{label} must not claim rejected output bytes")
            if event.get("rejection_classification") != "duplicate_input_bundle":
                errors.append(f"{label} rejection_classification must be duplicate_input_bundle")
            if event.get("exception_class") != "java.lang.IllegalArgumentException":
                errors.append(f"{label} exception_class must be java.lang.IllegalArgumentException")
            error_digest = event.get("error_message_sha256")
            if (
                not isinstance(error_digest, str)
                or SHA256_HEX_RE.fullmatch(error_digest) is None
                or error_digest == "0" * 64
            ):
                errors.append(f"{label} error_message_sha256 must be non-zero lowercase SHA-256")
            continue
        if event.get("outcome") != "succeeded":
            errors.append(f"{label} outcome must be succeeded")
        for key in ("rejection_classification", "exception_class", "error_message_sha256"):
            if event.get(key) is not None:
                errors.append(f"{label} {key} must be null for success")
        if operation in null_output_operations:
            if output_digest is not None or output_size != 0:
                errors.append(f"{label} must have null output and zero output size")
        else:
            if (
                not isinstance(output_digest, str)
                or SHA256_HEX_RE.fullmatch(output_digest) is None
                or output_digest == "0" * 64
            ):
                errors.append(f"{label} output_sha256 must be non-zero lowercase SHA-256")
            if isinstance(output_size, bool) or not isinstance(output_size, int) or output_size <= 0:
                errors.append(f"{label} output_size_bytes must be a positive integer")

    if not all(isinstance(event, dict) for event in value):
        return

    def event_input(event_index: int, input_index: int) -> str | None:
        inputs = value[event_index].get("input_sha256")
        if not isinstance(inputs, list) or input_index >= len(inputs):
            return None
        digest = inputs[input_index]
        return digest if isinstance(digest, str) else None

    def event_output(event_index: int) -> str | None:
        digest = value[event_index].get("output_sha256")
        return digest if isinstance(digest, str) else None

    output_links = (
        (1, 2, 0, "init request must feed init"),
        (3, 4, 0, "first append request must feed first append"),
        (5, 6, 0, "second append request must feed second append"),
        (2, 8, 0, "persisted init result must be restored exactly"),
        (4, 9, 0, "persisted first append result must be restored exactly"),
        (6, 10, 0, "persisted second append result must be restored exactly"),
        (16, 17, 0, "first verify request must feed first verify"),
        (18, 19, 0, "multi-hop verify request must feed multi-hop verify"),
        (20, 21, 0, "observed duplicate request must feed duplicate rejection"),
        (22, 23, 0, "first redemption request must feed first redemption"),
        (24, 25, 0, "second redemption request must feed second redemption"),
        (26, 27, 0, "sender-change redemption request must feed redemption"),
    )
    for output_event, input_event, input_index, description in output_links:
        output_digest = event_output(output_event)
        input_digest = event_input(input_event, input_index)
        if output_digest is not None and input_digest is not None and output_digest != input_digest:
            errors.append(f"candidate lifecycle causal linkage failed: {description}")

    # (source event, source input, consumer event, consumer input, semantic label)
    input_links = (
        # Restored init branch projected into the exact first append and restart validation.
        (3, 0, 11, 0, "init bundle projection"),
        (3, 1, 11, 1, "init provenance projection"),
        (3, 3, 11, 2, "init membership projection"),
        (3, 2, 11, 3, "init opening projection"),
        # Restored hop-one change projected into the exact second append and validation.
        (5, 0, 12, 0, "hop-one change bundle projection"),
        (5, 1, 12, 1, "hop-one change provenance projection"),
        (5, 3, 12, 2, "hop-one change membership projection"),
        (5, 2, 12, 3, "hop-one change opening projection"),
        # First recipient branch and original recipient request feed verification.
        (13, 0, 16, 0, "first recipient verify bundle"),
        (13, 1, 16, 2, "first recipient verify provenance"),
        (4, 1, 16, 1, "first recipient request projection"),
        # Second recipient branch and original recipient request feed verification.
        (14, 0, 18, 0, "multi-hop recipient verify bundle"),
        (14, 1, 18, 2, "multi-hop recipient verify provenance"),
        (6, 1, 18, 1, "multi-hop recipient request projection"),
        # The duplicate request must be built from the exact observed first-recipient branch.
        (13, 0, 20, 0, "duplicate observed bundle"),
        (13, 1, 20, 1, "duplicate observed provenance"),
        (13, 3, 20, 2, "duplicate observed opening"),
        (13, 2, 20, 3, "duplicate observed membership"),
        (13, 0, 21, 2, "duplicate rejection source bundle"),
        # Each redemption builder must project the exact validated branch.
        (13, 0, 22, 0, "first redemption bundle"),
        (13, 1, 22, 1, "first redemption provenance"),
        (13, 3, 22, 2, "first redemption opening"),
        (13, 2, 22, 3, "first redemption membership"),
        (14, 0, 24, 0, "second redemption bundle"),
        (14, 1, 24, 1, "second redemption provenance"),
        (14, 3, 24, 2, "second redemption opening"),
        (14, 2, 24, 3, "second redemption membership"),
        (15, 0, 26, 0, "sender-change redemption bundle"),
        (15, 1, 26, 1, "sender-change redemption provenance"),
        (15, 3, 26, 2, "sender-change redemption opening"),
        (15, 2, 26, 3, "sender-change redemption membership"),
        # Common redemption recipient and verifier commitment are exact assets.
        (22, 4, 24, 4, "common redemption recipient"),
        (22, 4, 26, 4, "common sender-change redemption recipient"),
        (22, 6, 24, 6, "common redemption verifier commitment"),
        (22, 6, 26, 6, "common sender-change verifier commitment"),
    )
    for source_event, source_input, target_event, target_input, description in input_links:
        source_digest = event_input(source_event, source_input)
        target_digest = event_input(target_event, target_input)
        if source_digest is not None and target_digest is not None and source_digest != target_digest:
            errors.append(f"candidate lifecycle causal linkage failed: {description}")


def _validate_candidate_lifecycle_transcript_v2(
    slot_path: Path,
    relative: str,
    expected_sha256: str,
    binding: dict[str, Any],
    metadata: dict[str, Any],
    errors: list[str],
) -> None:
    actual_sha256, digest_errors = _signed_evidence_artifact_sha256(slot_path, relative)
    if digest_errors:
        errors.extend(digest_errors)
        return
    if actual_sha256 != expected_sha256:
        errors.append("candidate lifecycle transcript digest does not match its file")
        return
    transcript = _load_json(
        slot_path / relative, "candidate lifecycle transcript", errors
    )
    if transcript is None:
        return
    for field in sorted(set(transcript) - KAGEMUSHA_CANDIDATE_LIFECYCLE_FIELDS_V2):
        errors.append(
            f"candidate lifecycle transcript contains unexpected field {_display_path(field)}"
        )
    for field in sorted(KAGEMUSHA_CANDIDATE_LIFECYCLE_FIELDS_V2 - set(transcript)):
        errors.append(
            f"candidate lifecycle transcript is missing field {_display_path(field)}"
        )
    if transcript.get("schema") != KAGEMUSHA_CANDIDATE_LIFECYCLE_SCHEMA_V2:
        errors.append(
            "candidate lifecycle transcript schema must be "
            f"{KAGEMUSHA_CANDIDATE_LIFECYCLE_SCHEMA_V2}"
        )
    expected_bindings = {
        "slot_id": metadata.get("slot_id"),
        "candidate_record_sha256": binding.get("candidate_record_sha256"),
        "candidate_manifest_sha256": binding.get("candidate_manifest_sha256"),
        "candidate_stage_manifest_path": binding.get("candidate_stage_manifest_path"),
        "candidate_stage_manifest_sha256": binding.get(
            "candidate_stage_manifest_sha256"
        ),
        "candidate_inventory_sha256": binding.get("native_accepted_inventory_sha256"),
        "source_commit": binding.get("source_commit"),
        "source_tree_sha256": binding.get("source_tree_sha256"),
        "source_repo_dirty": False,
        "generation": binding.get("generation"),
        "bridge_abi_version": REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION,
        "production_capability_observed": False,
        "attestation_challenge_sha256": metadata.get("attestation_challenge_sha256"),
        "attestation_certificate_chain_sha256": metadata.get(
            "attestation_certificate_chain_sha256"
        ),
        "app_signing_certificate_sha256": metadata.get(
            "app_signing_certificate_sha256"
        ),
        "strongbox_attestation": True,
        "physical_device_attestation": True,
    }
    for key, expected in expected_bindings.items():
        if transcript.get(key) != expected:
            errors.append(
                f"candidate lifecycle transcript {key} must match the accepted candidate binding"
            )
    atomic_values: dict[str, int] = {}
    for key in (
        "initial_atomic",
        "first_recipient_atomic",
        "second_recipient_atomic",
        "sender_change_atomic",
        "redeemed_atomic",
        "final_unspent_atomic",
    ):
        value = transcript.get(key)
        if not isinstance(value, str) or re.fullmatch(r"(?:0|[1-9][0-9]*)", value) is None:
            errors.append(f"candidate lifecycle transcript {key} must be canonical decimal atomic units")
            continue
        atomic_values[key] = int(value)
    if len(atomic_values) == 6:
        if atomic_values["initial_atomic"] <= 0:
            errors.append("candidate lifecycle transcript initial_atomic must be positive")
        if (
            atomic_values["first_recipient_atomic"]
            + atomic_values["second_recipient_atomic"]
            + atomic_values["sender_change_atomic"]
            != atomic_values["initial_atomic"]
        ):
            errors.append("candidate lifecycle transcript does not conserve exact atomic value")
        if atomic_values["redeemed_atomic"] != atomic_values["initial_atomic"]:
            errors.append("candidate lifecycle transcript must redeem the complete initial value")
        if atomic_values["final_unspent_atomic"] != 0:
            errors.append("candidate lifecycle transcript must finish with zero unspent atomic value")
    hops = transcript.get("proof_hops")
    if isinstance(hops, bool) or not isinstance(hops, int) or not 2 <= hops <= 8:
        errors.append("candidate lifecycle transcript proof_hops must be an integer from 2 through 8")
    for key in (
        "init_proof_verified",
        "first_spend_verified",
        "multi_hop_proof_verified",
        "independent_branch_redemption_verified",
        "duplicate_rejected",
        "restart_recovered",
    ):
        if transcript.get(key) is not True:
            errors.append(f"candidate lifecycle transcript {key} must be true")
    network_requests = transcript.get("network_requests_during_peer_transfers")
    if isinstance(network_requests, bool) or network_requests != 0:
        errors.append(
            "candidate lifecycle transcript network_requests_during_peer_transfers must be zero"
        )
    _validate_candidate_causal_events_v1(transcript.get("causal_events"), errors)


def validate_candidate_binding_v2(
    slot_path: Path,
    metadata: dict[str, Any],
    errors: list[str],
) -> dict[str, Any]:
    """Authenticate exact candidate, binary, inventory, and lifecycle file bindings."""

    details: dict[str, Any] = {}
    binding_relative = metadata.get("candidate_binding_path")
    if binding_relative != KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH:
        errors.append(
            "slot.json candidate_binding_path must be "
            f"{KAGEMUSHA_CANDIDATE_BINDING_ARTIFACT_PATH}"
        )
        return details
    binding_sha256 = metadata.get("candidate_binding_sha256")
    if (
        not isinstance(binding_sha256, str)
        or SHA256_HEX_RE.fullmatch(binding_sha256) is None
        or binding_sha256 == "0" * 64
    ):
        errors.append("slot.json candidate_binding_sha256 must be non-zero lowercase sha256 hex")
        return details
    actual_binding_sha256, digest_errors = _signed_evidence_artifact_sha256(
        slot_path, binding_relative
    )
    if digest_errors:
        errors.extend(digest_errors)
        return details
    if actual_binding_sha256 != binding_sha256:
        errors.append("slot.json candidate_binding_sha256 does not match candidate_binding_path")
        return details
    binding = _load_json(slot_path / binding_relative, "candidate binding", errors)
    if binding is None:
        return details
    for field in sorted(set(binding) - KAGEMUSHA_CANDIDATE_BINDING_FIELDS_V2):
        errors.append(f"candidate binding contains unexpected field {_display_path(field)}")
    missing = sorted(KAGEMUSHA_CANDIDATE_BINDING_FIELDS_V2 - set(binding))
    for field in missing:
        errors.append(f"candidate binding is missing field {_display_path(field)}")
    if binding.get("schema") != KAGEMUSHA_CANDIDATE_BINDING_SCHEMA_V2:
        errors.append(f"candidate binding schema must be {KAGEMUSHA_CANDIDATE_BINDING_SCHEMA_V2}")

    source_commit = _candidate_binding_string(binding, "source_commit", errors)
    if source_commit is not None and (
        re.fullmatch(r"[0-9a-f]{40}", source_commit) is None
        or source_commit == "0" * 40
    ):
        errors.append("candidate binding source_commit must be non-zero lowercase 40-byte git hex")
    generation = _candidate_binding_string(binding, "generation", errors)
    if generation is not None and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]{0,127}", generation) is None:
        errors.append("candidate binding generation must be a portable identifier")
    digest_fields = (
        "candidate_record_sha256",
        "candidate_manifest_sha256",
        "candidate_stage_manifest_sha256",
        "source_tree_sha256",
        "lab_native_library_sha256",
        "lab_apk_sha256",
        "lab_apk_signing_cert_sha256",
        "lab_test_apk_sha256",
        "lab_test_apk_signing_cert_sha256",
        "native_accepted_candidate_record_sha256",
        "native_accepted_candidate_manifest_sha256",
        "native_accepted_source_tree_sha256",
        "native_accepted_inventory_sha256",
        "lifecycle_transcript_sha256",
    )
    for key in digest_fields:
        _candidate_binding_sha256(binding, key, errors)
    if binding.get("bridge_abi_version") != REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION:
        errors.append("candidate binding bridge_abi_version must be 21")
    if binding.get("native_accepted_bridge_abi_version") != REQUIRED_KAGEMUSHA_NATIVE_BRIDGE_ABI_VERSION:
        errors.append("candidate binding native_accepted_bridge_abi_version must be 21")
    if binding.get("production_capability_observed") is not False:
        errors.append("candidate binding production_capability_observed must be false")
    if binding.get("source_repo_dirty") is not False:
        errors.append("candidate binding source_repo_dirty must be false")
    if binding.get("native_accepted_source_repo_dirty") is not False:
        errors.append("candidate binding native_accepted_source_repo_dirty must be false")

    equality_pairs = (
        ("candidate_record_sha256", "native_accepted_candidate_record_sha256"),
        ("candidate_manifest_sha256", "native_accepted_candidate_manifest_sha256"),
        ("source_commit", "native_accepted_source_commit"),
        ("source_tree_sha256", "native_accepted_source_tree_sha256"),
        ("source_repo_dirty", "native_accepted_source_repo_dirty"),
        ("generation", "native_accepted_generation"),
        ("bridge_abi_version", "native_accepted_bridge_abi_version"),
    )
    for expected_key, accepted_key in equality_pairs:
        if binding.get(expected_key) != binding.get(accepted_key):
            errors.append(
                f"candidate binding {accepted_key} must match {expected_key}"
            )

    metadata_bindings = {
        "candidate_record_path": "candidate_record_path",
        "candidate_record_sha256": "candidate_record_sha256",
        "candidate_manifest_path": "candidate_manifest_path",
        "candidate_manifest_sha256": "candidate_manifest_sha256",
        "candidate_stage_manifest_path": "candidate_stage_manifest_path",
        "candidate_stage_manifest_sha256": "candidate_stage_manifest_sha256",
        "source_commit": "candidate_source_commit",
        "source_tree_sha256": "candidate_source_tree_sha256",
        "source_repo_dirty": "candidate_source_repo_dirty",
        "generation": "candidate_generation",
        "lab_native_library_path": "candidate_lab_native_library_path",
        "lab_native_library_sha256": "candidate_lab_native_library_sha256",
        "lab_apk_path": "candidate_lab_apk_path",
        "lab_apk_sha256": "candidate_lab_apk_sha256",
        "lab_apk_signing_cert_sha256": (
            "candidate_lab_apk_signing_certificate_sha256"
        ),
        "lab_test_apk_path": "candidate_lab_test_apk_path",
        "lab_test_apk_sha256": "candidate_lab_test_apk_sha256",
        "lab_test_apk_signing_cert_sha256": (
            "candidate_lab_test_apk_signing_certificate_sha256"
        ),
        "lifecycle_transcript_path": "candidate_lifecycle_transcript_path",
        "lifecycle_transcript_sha256": "candidate_lifecycle_transcript_sha256",
        "native_accepted_inventory_sha256": "candidate_inventory_sha256",
        "production_capability_observed": "production_capability_observed",
    }
    for binding_key, metadata_key in metadata_bindings.items():
        if binding.get(binding_key) != metadata.get(metadata_key):
            errors.append(
                f"candidate binding {binding_key} must match slot.json {metadata_key}"
            )

    file_bindings = (
        ("candidate_record_path", "candidate_record_sha256", None),
        ("candidate_manifest_path", "candidate_manifest_sha256", None),
        ("lab_native_library_path", "lab_native_library_sha256", ".so"),
        ("lab_apk_path", "lab_apk_sha256", ".apk"),
        ("lab_test_apk_path", "lab_test_apk_sha256", ".apk"),
    )
    bound_paths = [binding.get(path_key) for path_key, _, _ in file_bindings]
    if any(not isinstance(path, str) for path in bound_paths) or len(set(bound_paths)) != len(
        bound_paths
    ):
        errors.append(
            "candidate binding candidate, manifest, native library, main APK, and test APK paths must be distinct"
        )
    if binding.get("candidate_record_sha256") == binding.get("candidate_manifest_sha256"):
        errors.append("candidate binding candidate and manifest digests must be distinct")
    for path_key, digest_key, suffix in file_bindings:
        relative = _candidate_binding_path(binding, path_key, errors)
        expected_digest = binding.get(digest_key)
        if relative is None or not isinstance(expected_digest, str):
            continue
        if suffix is not None and not relative.endswith(suffix):
            errors.append(f"candidate binding {path_key} must end in {suffix}")
            continue
        actual_digest, file_errors = _signed_evidence_artifact_sha256(slot_path, relative)
        if file_errors:
            errors.extend(file_errors)
        elif actual_digest != expected_digest:
            errors.append(f"candidate binding {digest_key} does not match {path_key}")
    if binding.get("lab_apk_path") == metadata.get("kagemusha_wallet_apk_path"):
        errors.append("candidate lab APK path must be distinct from the wallet APK path")
    if binding.get("lab_apk_sha256") == metadata.get("kagemusha_wallet_apk_sha256"):
        errors.append("candidate lab APK digest must be distinct from the wallet APK digest")
    if binding.get("lab_test_apk_sha256") == metadata.get("kagemusha_wallet_apk_sha256"):
        errors.append("candidate lab test APK digest must be distinct from the wallet APK digest")
    if binding.get("lab_apk_sha256") == binding.get("lab_test_apk_sha256"):
        errors.append("candidate lab main and test APK digests must be distinct")
    if binding.get("lab_apk_signing_cert_sha256") != binding.get(
        "lab_test_apk_signing_cert_sha256"
    ):
        errors.append("candidate lab main and test APK signing certificates must match")
    if binding.get("lab_apk_signing_cert_sha256") == metadata.get(
        "app_signing_certificate_sha256"
    ):
        errors.append("candidate lab signer must be distinct from the attested wallet signer")

    expected_main_apk = (
        "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
        f"{binding.get('candidate_record_sha256')}-debug.apk"
    )
    expected_test_apk = (
        "evidence/kagemusha-candidate-evidence-lab-DO-NOT-SHIP-"
        f"{binding.get('candidate_record_sha256')}-debug-androidTest.apk"
    )
    if binding.get("lab_apk_path") != expected_main_apk:
        errors.append("candidate binding lab_apk_path must use the exact marker-bearing name")
    if binding.get("lab_test_apk_path") != expected_test_apk:
        errors.append("candidate binding lab_test_apk_path must use the exact marker-bearing name")

    for path_key, certificate_key in (
        ("lab_apk_path", "lab_apk_signing_cert_sha256"),
        ("lab_test_apk_path", "lab_test_apk_signing_cert_sha256"),
    ):
        relative = binding.get(path_key)
        if not isinstance(relative, str):
            continue
        normalized = _normalise_safe_relative_path(
            relative, errors, f"candidate binding {path_key}"
        )
        if normalized is None or not _safe_relative_path_is_child_of(
            normalized, "evidence"
        ):
            continue
        try:
            forbidden_krv4_entries = _candidate_lab_apk_forbidden_krv4_entries(
                slot_path / normalized
            )
        except ValueError as error:
            errors.append(f"candidate binding {path_key} packaging is invalid: {error}")
        else:
            if forbidden_krv4_entries:
                errors.append(
                    f"candidate binding {path_key} embeds external KRV4 artifact entries"
                )
        try:
            measured_certificate = extract_apk_signing_certificate_sha256(
                slot_path / normalized
            )
        except (OSError, ValueError) as error:
            errors.append(f"candidate binding {path_key} signer is invalid: {error}")
        else:
            if measured_certificate != binding.get(certificate_key):
                errors.append(
                    f"candidate binding {certificate_key} does not match APK signer DER"
                )

    stage_path = binding.get("candidate_stage_manifest_path")
    if stage_path != KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2:
        errors.append(
            "candidate binding candidate_stage_manifest_path must be "
            f"{KAGEMUSHA_CANDIDATE_STAGE_MANIFEST_PATH_V2}"
        )
    else:
        try:
            validate_kagemusha_candidate_stage_manifest_v2(
                slot_path,
                candidate_sha256=str(binding.get("candidate_record_sha256")),
                stage_sha256=str(binding.get("candidate_stage_manifest_sha256")),
                source_commit=str(binding.get("source_commit")),
                source_tree_sha256=str(binding.get("source_tree_sha256")),
            )
        except (OSError, ValueError) as error:
            errors.append(f"candidate stage manifest validation failed: {error}")

    for key in (
        "candidate_source_tree_sha256_before",
        "candidate_source_tree_sha256_after",
    ):
        if metadata.get(key) != binding.get("source_tree_sha256"):
            errors.append(f"slot.json {key} must equal the accepted source-tree seal")

    raw_inventory = binding.get("artifact_inventory")
    measured_inventory: list[dict[str, Any]] = []
    if not isinstance(raw_inventory, list) or len(raw_inventory) != 8:
        errors.append("candidate binding artifact_inventory must contain exactly eight entries")
    else:
        seen_paths: set[str] = {
            path for path in bound_paths if isinstance(path, str)
        }
        for index, (entry, expected_role, expected_name) in enumerate(
            zip(
                raw_inventory,
                KAGEMUSHA_CANDIDATE_ARTIFACT_ROLES_V4,
                KAGEMUSHA_CANDIDATE_ARTIFACT_FILE_NAMES_V4,
            )
        ):
            if not isinstance(entry, dict):
                errors.append(f"candidate binding artifact_inventory[{index}] must be an object")
                continue
            for field in sorted(set(entry) - KAGEMUSHA_CANDIDATE_ARTIFACT_ENTRY_FIELDS_V2):
                errors.append(
                    f"candidate binding artifact_inventory[{index}] contains unexpected field {_display_path(field)}"
                )
            if set(entry) != KAGEMUSHA_CANDIDATE_ARTIFACT_ENTRY_FIELDS_V2:
                errors.append(
                    f"candidate binding artifact_inventory[{index}] must have the exact V2 fields"
                )
            if entry.get("role") != expected_role:
                errors.append(
                    f"candidate binding artifact_inventory[{index}] role must be {expected_role}"
                )
            relative = entry.get("path")
            if not isinstance(relative, str):
                errors.append(f"candidate binding artifact_inventory[{index}] path must be text")
                continue
            normalized = _normalise_safe_relative_path(
                relative, errors, f"candidate binding artifact_inventory[{index}] path"
            )
            if (
                normalized is None
                or not _safe_relative_path_is_child_of(normalized, "evidence")
                or PurePosixPath(normalized).name != expected_name
            ):
                errors.append(
                    f"candidate binding artifact_inventory[{index}] path must bind {expected_name} under evidence/"
                )
                continue
            if normalized in seen_paths:
                errors.append("candidate binding artifact_inventory paths must be unique")
                continue
            seen_paths.add(normalized)
            measured = _candidate_artifact_measurement(slot_path, normalized, errors)
            if measured is None:
                continue
            for key in (
                "framed_size_bytes",
                "framed_sha256",
                "payload_size_bytes",
                "payload_sha256",
            ):
                if entry.get(key) != measured[key]:
                    errors.append(
                        f"candidate binding artifact_inventory[{index}] {key} does not match the KRV4 file"
                    )
            measured_inventory.append({"role": expected_role, **measured})
    if len(measured_inventory) == 8:
        inventory_sha256 = _candidate_inventory_sha256(measured_inventory)
        if inventory_sha256 != binding.get("native_accepted_inventory_sha256"):
            errors.append("candidate binding native_accepted_inventory_sha256 is not exact")
        if inventory_sha256 != metadata.get("candidate_inventory_sha256"):
            errors.append("slot.json candidate_inventory_sha256 is not exact")
        details["candidate_inventory_sha256"] = inventory_sha256

    lifecycle_relative = _candidate_binding_path(
        binding, "lifecycle_transcript_path", errors
    )
    lifecycle_sha256 = binding.get("lifecycle_transcript_sha256")
    if lifecycle_relative != KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH:
        errors.append(
            "candidate binding lifecycle_transcript_path must be "
            f"{KAGEMUSHA_CANDIDATE_LIFECYCLE_TRANSCRIPT_PATH}"
        )
    elif isinstance(lifecycle_sha256, str):
        _validate_candidate_lifecycle_transcript_v2(
            slot_path,
            lifecycle_relative,
            lifecycle_sha256,
            binding,
            metadata,
            errors,
        )

    for field in (
        "candidate_binding_path",
        "candidate_record_path",
        "candidate_record_sha256",
        "candidate_manifest_path",
        "candidate_manifest_sha256",
        "candidate_stage_manifest_path",
        "candidate_stage_manifest_sha256",
        "candidate_source_commit",
        "candidate_source_tree_sha256",
        "candidate_source_tree_sha256_before",
        "candidate_source_tree_sha256_after",
        "candidate_generation",
        "candidate_lab_native_library_path",
        "candidate_lab_native_library_sha256",
        "candidate_lab_apk_path",
        "candidate_lab_apk_sha256",
        "candidate_lab_apk_signing_certificate_sha256",
        "candidate_lab_test_apk_path",
        "candidate_lab_test_apk_sha256",
        "candidate_lab_test_apk_signing_certificate_sha256",
        "candidate_lifecycle_transcript_path",
        "candidate_lifecycle_transcript_sha256",
        "candidate_inventory_sha256",
        "attestation_challenge_sha256",
        "attestation_certificate_chain_sha256",
        "app_signing_certificate_sha256",
    ):
        value = metadata.get(field)
        if isinstance(value, str):
            details[field] = value
    details["candidate_binding_sha256"] = binding_sha256
    details["production_capability_observed"] = False
    details["candidate_source_repo_dirty"] = False
    return details


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
    for key in SIGNED_EVIDENCE_SLOT_FALSE_FIELDS:
        _require_evidence_false(evidence, key, errors)
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
    attestation_certificate_count: int | None = None
    validate_slot_metadata_fields(metadata, errors)
    if metadata.get("schema") != KAGEMUSHA_SLOT_SCHEMA_V2:
        errors.append(
            "slot.json schema must be candidate-bound "
            f"{KAGEMUSHA_SLOT_SCHEMA_V2}; V1 evidence is not production evidence"
        )
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
                attestation_certificate_count = (
                    _validate_android_attestation_certificate_chain(
                        chain_relative,
                        chain_bytes,
                        metadata,
                        errors,
                    )
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
        "kagemusha_wallet_policy_sha256",
        "slot.json",
        errors,
    )
    apk_digest = _require_lowercase_sha256_hex(
        metadata,
        "kagemusha_wallet_apk_sha256",
        "slot.json",
        errors,
    )
    apk_relative = _require_non_empty_string(metadata, "kagemusha_wallet_apk_path", errors)
    if apk_relative is not None:
        apk_relative = _normalise_safe_relative_path(
            apk_relative,
            errors,
            "slot.json kagemusha_wallet_apk_path",
        )
    if apk_relative is not None:
        if not _safe_relative_path_is_child_of(apk_relative, "evidence"):
            errors.append("slot.json kagemusha_wallet_apk_path must stay under evidence/")
        else:
            _, actual_apk_digest, digest_errors = _metadata_artifact_bytes_and_sha256(
                slot_path,
                apk_relative,
                "slot.json kagemusha_wallet_apk_path",
                "slot.json kagemusha_wallet_apk_path must point to an existing file",
                _slot_artifact_max_bytes(apk_relative),
            )
            if digest_errors:
                errors.extend(digest_errors)
            elif apk_digest is not None and actual_apk_digest is not None:
                if actual_apk_digest != apk_digest:
                    errors.append(
                        "slot.json kagemusha_wallet_apk_sha256 does not match kagemusha_wallet_apk_path"
                    )
                else:
                    details["kagemusha_wallet_apk_path"] = apk_relative
                    details["kagemusha_wallet_apk_sha256"] = apk_digest
                try:
                    wallet_signer = extract_apk_signing_certificate_sha256(
                        slot_path / apk_relative
                    )
                except (OSError, ValueError) as error:
                    errors.append(
                        "production wallet APK signature verification failed: "
                        f"{error}"
                    )
                else:
                    if wallet_signer != metadata.get(
                        "app_signing_certificate_sha256"
                    ):
                        errors.append(
                            "slot.json app_signing_certificate_sha256 must match the verified production wallet APK signer"
                        )

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
    _require_false(metadata, "production_capability_observed", errors)
    _require_status(metadata, "kagemusha_recursive_spend_ffi_surface", {"passed"}, errors)
    _require_status(
        metadata,
        "kagemusha_recursive_spend_jni_probe",
        KAGEMUSHA_RECURSIVE_SPEND_JNI_PROBE_STATES,
        errors,
    )
    _require_status(
        metadata,
        "kagemusha_recursive_spend_prover_state",
        KAGEMUSHA_RECURSIVE_SPEND_PROVER_STATES,
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
    candidate_details = validate_candidate_binding_v2(slot_path, metadata, errors)
    details.update(candidate_details)
    if attestation_certificate_count is not None:
        details["strongbox_attestation"] = True
        details["physical_device_attestation"] = True
        authority_projection = android_evidence_authority_projection()
        if authority_projection is not None:
            details["authority_tools"] = authority_projection
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
        attestation_certificate_count=attestation_certificate_count,
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
        covered_d2d_payment_transports_by_family = (
            _summary_release_d2d_payment_transport_coverage_by_family(
                summary_reports,
                trusted_signer_public_key_sha256,
            )
        )
        missing_d2d_payment_transport_pairs = (
            _missing_summary_release_d2d_payment_transport_pairs(
                covered_d2d_payment_transports_by_family,
            )
        )
        summary["kagemusha"] = {
            "production_evidence_required": require_kagemusha_production_evidence,
            "standard_matrix_required": require_kagemusha_standard_matrix,
            "authority_tools": android_evidence_authority_projection(),
            "required_device_families": list(KAGEMUSHA_STANDARD_DEVICE_FAMILIES),
            "covered_device_families": covered,
            "missing_device_families": missing,
            "required_d2d_payment_transports": sorted(D2D_PAYMENT_TRANSPORTS),
            "covered_d2d_payment_transports": covered_d2d_payment_transports,
            "missing_d2d_payment_transports": missing_d2d_payment_transports,
            "covered_d2d_payment_transports_by_family": covered_d2d_payment_transports_by_family,
            "missing_d2d_payment_transport_pairs": missing_d2d_payment_transport_pairs,
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
    """Return duplicated release matrix bindings without exposing raw values."""

    duplicates: dict[str, list[dict[str, Any]]] = {}
    for field in (
        "device_fingerprint_sha256",
        "attestation_challenge_sha256",
        "d2d_payment_transcript_sha256",
    ):
        seen: dict[str, set[str]] = {}
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
            if not isinstance(slot, str):
                continue
            for value in _summary_duplicate_matrix_values(kagemusha, field):
                seen.setdefault(value, set()).add(_display_slot_name(slot))
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


KAGEMUSHA_ANDROID_CONFIRMATION_COMPARISON_SCHEMA_V1 = (
    "iroha.android.device_lab.kagemusha.confirmation_comparison.v1"
)


def _read_confirmation_json_artifact(
    path: Path,
    label: str,
) -> tuple[dict[str, Any] | None, dict[str, Any] | None, list[str]]:
    errors: list[str] = []
    path_text = os.fspath(path)
    if not path.is_absolute() or path_text != path_text.strip():
        return None, None, [f"{label} must be one canonical absolute path"]
    try:
        canonical = path.resolve(strict=True)
        initial = path.lstat()
    except OSError:
        return None, None, [f"{label} could not be inspected"]
    if canonical != path or stat.S_ISLNK(initial.st_mode):
        return None, None, [f"{label} must be a canonical non-symlink path"]
    if not stat.S_ISREG(initial.st_mode) or initial.st_nlink != 1:
        return None, None, [f"{label} must be a single-link regular file"]
    if initial.st_uid not in {0, os.geteuid()} or initial.st_mode & 0o077:
        return None, None, [f"{label} must be owner-private and owned by root or the invoking user"]
    if initial.st_size <= 0 or initial.st_size > MAX_ANDROID_DEVICE_LAB_JSON_BYTES:
        return None, None, [f"{label} has an invalid file size"]
    flags = os.O_RDONLY
    if hasattr(os, "O_NOFOLLOW"):
        flags |= os.O_NOFOLLOW
    chunks: list[bytes] = []
    digest = hashlib.sha256()
    try:
        descriptor = os.open(path, flags)
        try:
            opened = os.fstat(descriptor)
            identity = (initial.st_dev, initial.st_ino)
            if (
                (opened.st_dev, opened.st_ino) != identity
                or not stat.S_ISREG(opened.st_mode)
                or opened.st_nlink != 1
                or opened.st_size != initial.st_size
            ):
                return None, None, [f"{label} changed while being opened"]
            while True:
                chunk = os.read(descriptor, 1024 * 1024)
                if not chunk:
                    break
                chunks.append(chunk)
                digest.update(chunk)
            final = path.lstat()
            if (
                (final.st_dev, final.st_ino) != identity
                or final.st_size != opened.st_size
                or final.st_mtime_ns != initial.st_mtime_ns
                or final.st_ctime_ns != initial.st_ctime_ns
            ):
                return None, None, [f"{label} changed while being read"]
        finally:
            os.close(descriptor)
    except OSError:
        return None, None, [f"{label} could not be read"]
    payload = b"".join(chunks)
    try:
        document = _strict_json_object_bytes(payload, label)
    except ValueError as error:
        errors.append(str(error))
        return None, None, errors
    return document, {
        "path": path_text,
        "size_bytes": len(payload),
        "sha256": digest.hexdigest(),
    }, errors


def validate_kagemusha_android_confirmation(
    *,
    reference_slot: Path,
    confirmation_binding_path: Path,
    confirmation_lifecycle_path: Path,
    trusted_signer_public_keys: dict[str, Path],
) -> dict[str, Any]:
    """Compare an independent rerun with one fully authenticated reference slot."""

    errors: list[str] = []
    artifacts: dict[str, Any] = {}
    reference_report = scan_slot(
        reference_slot,
        require_kagemusha_production_evidence=True,
        trusted_signer_public_keys=trusted_signer_public_keys,
    )
    if reference_report.get("status") != "ok":
        errors.append("reference slot failed full authenticated production validation")

    metadata = _load_json(reference_slot / "slot.json", "reference slot.json", errors)
    reference_binding: dict[str, Any] | None = None
    reference_lifecycle: dict[str, Any] | None = None
    if metadata is not None:
        binding_relative = metadata.get("candidate_binding_path")
        lifecycle_relative = metadata.get("candidate_lifecycle_transcript_path")
        if isinstance(binding_relative, str):
            reference_binding, measurement, measurement_errors = (
                _read_confirmation_json_artifact(
                    (reference_slot / binding_relative).resolve(),
                    "reference candidate binding",
                )
            )
            errors.extend(measurement_errors)
            if measurement is not None:
                artifacts["reference_binding"] = measurement
        else:
            errors.append("reference slot has no candidate binding path")
        if isinstance(lifecycle_relative, str):
            reference_lifecycle, measurement, measurement_errors = (
                _read_confirmation_json_artifact(
                    (reference_slot / lifecycle_relative).resolve(),
                    "reference lifecycle transcript",
                )
            )
            errors.extend(measurement_errors)
            if measurement is not None:
                artifacts["reference_lifecycle"] = measurement
        else:
            errors.append("reference slot has no lifecycle transcript path")

    confirmation_binding, measurement, measurement_errors = (
        _read_confirmation_json_artifact(
            confirmation_binding_path,
            "confirmation candidate binding",
        )
    )
    errors.extend(measurement_errors)
    if measurement is not None:
        artifacts["confirmation_binding"] = measurement
    confirmation_lifecycle, lifecycle_measurement, measurement_errors = (
        _read_confirmation_json_artifact(
            confirmation_lifecycle_path,
            "confirmation lifecycle transcript",
        )
    )
    errors.extend(measurement_errors)
    if lifecycle_measurement is not None:
        artifacts["confirmation_lifecycle"] = lifecycle_measurement

    if reference_binding is not None and confirmation_binding is not None:
        if set(confirmation_binding) != KAGEMUSHA_CANDIDATE_BINDING_FIELDS_V2:
            errors.append("confirmation candidate binding does not have the exact V2 fields")
        if confirmation_binding.get("schema") != KAGEMUSHA_CANDIDATE_BINDING_SCHEMA_V2:
            errors.append("confirmation candidate binding schema is not V2")
        deterministic_binding_mismatch = False
        for key in sorted(set(reference_binding) | set(confirmation_binding)):
            if key == "lifecycle_transcript_sha256":
                continue
            if confirmation_binding.get(key) != reference_binding.get(key):
                deterministic_binding_mismatch = True
        if deterministic_binding_mismatch:
            errors.append(
                "confirmation candidate binding deterministic fields differ from reference"
            )
    if confirmation_binding is not None and lifecycle_measurement is not None:
        if confirmation_binding.get("lifecycle_transcript_sha256") != lifecycle_measurement.get(
            "sha256"
        ):
            errors.append(
                "confirmation candidate binding does not bind the pulled lifecycle transcript"
            )

    if (
        metadata is not None
        and confirmation_binding is not None
        and confirmation_lifecycle is not None
        and lifecycle_measurement is not None
    ):
        lifecycle_validation_errors: list[str] = []
        _validate_candidate_lifecycle_transcript_v2(
            confirmation_lifecycle_path.parent,
            confirmation_lifecycle_path.name,
            lifecycle_measurement["sha256"],
            confirmation_binding,
            metadata,
            lifecycle_validation_errors,
        )
        errors.extend(
            f"confirmation lifecycle validation failed: {error}"
            for error in lifecycle_validation_errors
        )

    if reference_lifecycle is not None and confirmation_lifecycle is not None:
        def without_durations(document: dict[str, Any]) -> dict[str, Any]:
            normalized = json.loads(json.dumps(document, allow_nan=False))
            events = normalized.get("causal_events")
            if isinstance(events, list):
                for event in events:
                    if isinstance(event, dict) and "duration_nanos" in event:
                        event["duration_nanos"] = "<allowed-to-differ>"
            return normalized

        if without_durations(confirmation_lifecycle) != without_durations(
            reference_lifecycle
        ):
            errors.append(
                "confirmation lifecycle differs from reference outside causal duration_nanos"
            )

    required_artifacts = {
        "reference_binding",
        "reference_lifecycle",
        "confirmation_binding",
        "confirmation_lifecycle",
    }
    if set(artifacts) != required_artifacts:
        errors.append("confirmation comparison did not measure all four required artifacts")
    return {
        "schema": KAGEMUSHA_ANDROID_CONFIRMATION_COMPARISON_SCHEMA_V1,
        "status": "ok" if not errors else "error",
        "errors": errors,
        "artifacts": artifacts,
        "comparison": {
            "deterministic_fields_equal": not errors,
            "only_duration_nanos_may_differ": True,
        },
        "authority_tools": android_evidence_authority_projection(),
    }


def main(argv: list[str] | None = None) -> int:
    global _ANDROID_EVIDENCE_AUTHORITY
    _ANDROID_EVIDENCE_AUTHORITY = None
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
    parser.add_argument(
        "--java",
        default=None,
        help="Canonical absolute Java executable used for APK verification.",
    )
    parser.add_argument(
        "--java-sha256",
        default=None,
        help="Pinned lowercase SHA-256 of --java.",
    )
    parser.add_argument(
        "--apksigner-jar",
        default=None,
        help="Canonical absolute Android build-tools apksigner.jar path.",
    )
    parser.add_argument(
        "--apksigner-jar-sha256",
        default=None,
        help="Pinned lowercase SHA-256 of --apksigner-jar.",
    )
    parser.add_argument(
        "--openssl",
        default=None,
        help="Canonical absolute OpenSSL executable path.",
    )
    parser.add_argument(
        "--openssl-sha256",
        default=None,
        help="Pinned lowercase SHA-256 of --openssl.",
    )
    parser.add_argument(
        "--android-attestation-trust-root",
        action="append",
        default=None,
        help="Canonical absolute DER/PEM Android attestation trust root (repeatable).",
    )
    parser.add_argument(
        "--android-attestation-trust-root-sha256",
        action="append",
        default=None,
        help="Aligned lowercase SHA-256 pin for each attestation trust root.",
    )
    parser.add_argument(
        "--android-attestation-revocation-status",
        default=None,
        help="Canonical absolute local Android attestation revocation-status JSON.",
    )
    parser.add_argument(
        "--android-attestation-revocation-status-sha256",
        default=None,
        help="Pinned lowercase SHA-256 of the revocation-status JSON.",
    )
    parser.add_argument(
        "--confirmation-reference-slot",
        default=None,
        help="Fully authenticated reference slot for an independent candidate rerun.",
    )
    parser.add_argument(
        "--confirmation-binding",
        default=None,
        help="Pulled candidate-binding-v2.json from the independent rerun.",
    )
    parser.add_argument(
        "--confirmation-lifecycle",
        default=None,
        help="Pulled lifecycle-transcript-v2.json from the independent rerun.",
    )
    parser.add_argument(
        "--confirmation-json-out",
        default=None,
        help="Required machine report for confirmation comparison mode.",
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

    confirmation_values = (
        args.confirmation_reference_slot,
        args.confirmation_binding,
        args.confirmation_lifecycle,
        args.confirmation_json_out,
    )
    if any(value is not None for value in confirmation_values):
        if any(value is None for value in confirmation_values):
            print(
                "[device-lab] confirmation mode requires reference slot, binding, lifecycle, and JSON output",
                file=sys.stderr,
            )
            return 1
        authority_values = (
            args.java,
            args.java_sha256,
            args.apksigner_jar,
            args.apksigner_jar_sha256,
            args.openssl,
            args.openssl_sha256,
            args.android_attestation_revocation_status,
            args.android_attestation_revocation_status_sha256,
        )
        if any(value is None for value in authority_values):
            print(
                "[device-lab] confirmation mode requires every digest-pinned authority path/digest pair",
                file=sys.stderr,
            )
            return 1
        authority_errors = _configure_android_evidence_authority_from_args(args)
        if authority_errors:
            for error in authority_errors:
                print(f"[device-lab] {error}", file=sys.stderr)
            return 1
        trusted_signer_public_keys, signer_errors = load_trusted_signer_public_keys(
            args.trusted_signer_public_keys
        )
        if signer_errors or not trusted_signer_public_keys:
            for error in signer_errors or [
                "confirmation mode requires at least one trusted signer public key"
            ]:
                print(f"[device-lab] {error}", file=sys.stderr)
            return 1
        comparison = validate_kagemusha_android_confirmation(
            reference_slot=Path(args.confirmation_reference_slot),
            confirmation_binding_path=Path(args.confirmation_binding),
            confirmation_lifecycle_path=Path(args.confirmation_lifecycle),
            trusted_signer_public_keys=trusted_signer_public_keys,
        )
        write_errors = write_summary(Path(args.confirmation_json_out), comparison)
        if write_errors:
            for error in write_errors:
                print(f"[device-lab] {error}", file=sys.stderr)
            return 1
        if comparison["status"] != "ok":
            print(
                "[device-lab] independent candidate confirmation differs from authenticated reference",
                file=sys.stderr,
            )
            return 1
        print("[device-lab] independent candidate confirmation: ok")
        return 0

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
    authority_values = (
        args.java,
        args.java_sha256,
        args.apksigner_jar,
        args.apksigner_jar_sha256,
        args.openssl,
        args.openssl_sha256,
        args.android_attestation_revocation_status,
        args.android_attestation_revocation_status_sha256,
    )
    authority_lists = (
        args.android_attestation_trust_root or [],
        args.android_attestation_trust_root_sha256 or [],
    )
    if any(value is not None for value in authority_values) or any(authority_lists):
        if any(value is None for value in authority_values):
            print(
                "[device-lab] all Java, apksigner.jar, openssl, and attestation revocation path/digest pairs are required",
                file=sys.stderr,
            )
            return 1
        authority_errors = _configure_android_evidence_authority_from_args(args)
        if authority_errors:
            for error in authority_errors:
                print(f"[device-lab] {error}", file=sys.stderr)
            return 1
    if require_kagemusha and _ANDROID_EVIDENCE_AUTHORITY is None:
        print(
            "[device-lab] production evidence requires explicit digest-pinned Android authority inputs",
            file=sys.stderr,
        )
        return 1
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
        covered_d2d_payment_transports_by_family = (
            _summary_release_d2d_payment_transport_coverage_by_family(
                reports,
                trusted_signer_public_key_sha256,
            )
        )
        missing_d2d_payment_transport_pairs = (
            _missing_summary_release_d2d_payment_transport_pairs(
                covered_d2d_payment_transports_by_family,
            )
        )
        if missing:
            failures += 1
            print(
                "[device-lab] missing Kagemusha production evidence for device families: "
                + ", ".join(missing),
                file=sys.stderr,
            )
        if missing_d2d_payment_transports or missing_d2d_payment_transport_pairs:
            failures += 1
            print(
                "[device-lab] missing Kagemusha production evidence for "
                "standard-family D2D payment transports: "
                + ", ".join(
                    f"{item['device_family']}={item['transport']}"
                    for item in missing_d2d_payment_transport_pairs
                ),
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
