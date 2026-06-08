#!/usr/bin/env python3
"""Verify archived ISO 20022 operator production evidence.

Purpose:
  This offline gate validates operator canary and trust-bundle summaries before
  they are archived as production evidence. It rejects plan-only canaries,
  dry-run child commands, insecure HTTP overrides, default-profile fallbacks,
  synthetic trust DER, record-only signature policy, digest tampering, skipped
  stages, failed stages, and obvious secret leakage.

Prerequisites:
  Python 3.11+. No third party Python packages are required. Canary summaries
  should be produced by ``iso_operator_canary.py`` and trust summaries should be
  produced by ``iso_trust_bundle_verify.py``.

Safety:
  The verifier is read-only. It never contacts provider, Torii, notary, OCSP,
  CRL, or rail endpoints. If ``--receipt`` or ``--receipt-dir`` is supplied it
  invokes the local receipt verifier in read-only mode.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import datetime as dt
import errno
import hashlib
import ipaddress
import json
import math
import os
import re
import secrets
import stat
import subprocess
import sys
import threading
import urllib.parse
from pathlib import Path
from typing import Any


EVIDENCE_VERSION = 1
CANARY_SUMMARY_VERSION = 1
RECEIPT_SUMMARY_VERSION = 1
TRUST_SUMMARY_VERSION = 1
REQUIRE_VERIFIED = "require-verified"
TRUST_SIGNATURE_POLICIES = {"record-only", "reject-unsupported", REQUIRE_VERIFIED}
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.\d{3}$")
RAIL_MESSAGE_ID_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._:@+-]*[A-Za-z0-9])?$")
KNOWN_RAILS = {
    "generic-iso20022",
    "swift-cbpr-plus",
    "fedwire-funds",
    "sepa-sct-inst",
    "securities-csd",
}
EXPECTED_CANARY_STAGE_ORDER = ("rail", "notary", "verify")
REQUIRED_CANARY_STAGES = set(EXPECTED_CANARY_STAGE_ORDER)
REQUIRED_RECEIPT_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
RECEIPT_PATH_SUFFIX = ".receipt.json"
SUMMARY_DIGEST_FIELD = "summary_sha256"
LEGACY_RAIL_MESSAGE_TYPES = {"colr.007"}
SUPPORTED_RAIL_MESSAGE_TYPES = {
    "pacs.008",
    "pacs.009",
    "pacs.002",
    "pacs.004",
    "camt.056",
    "sese.023",
    "sese.024",
    "sese.025",
    "colr.007",
    "colr.012",
}
MAX_TRUST_DER_BLOBS = 8
MAX_TRUST_DER_BYTES = 1024 * 1024
MAX_TRUST_DER_BASE64_CHARS = ((MAX_TRUST_DER_BYTES + 2) // 3) * 4
MAX_RAIL_MESSAGE_ID_CHARS = 128
MAX_SUMMARY_JSON_BYTES = 4 * 1024 * 1024
MAX_RECEIPT_VERIFIER_OUTPUT_BYTES = 4 * 1024 * 1024
MAX_HTTP_URL_CHARS = 2048
DEFAULT_RECEIPT_VERIFIER_TIMEOUT_SECS = 300.0
PLACEHOLDER_TRUST_SOURCE_MARKERS = (
    "dummy",
    "fake",
    "placeholder",
    "replace-before-production",
    "sample",
    "template",
)
PLACEHOLDER_TRUST_SOURCE_HOSTS = {
    "example",
    "example.com",
    "example.invalid",
    "example.net",
    "operator-canary.bank",
    "example.org",
}
TEMPLATE_CANARY_ENDPOINT_HOSTS = {
    "operator-canary.bank",
}
LOCAL_REBINDING_HOST_SUFFIXES = {"localtest.me", "lvh.me", "nip.io", "sslip.io", "vcap.me"}
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
SCRIPT_DIR = Path(__file__).resolve().parent

EXPECTED_STAGE_SCRIPTS = {
    "rail": "iso_rail_gateway_adapter.py",
    "notary": "iso_audit_notary_adapter.py",
    "verify": "iso_operator_receipt_verify.py",
}
EXPECTED_STAGE_FLAGS = {
    "rail": {
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--bearer-token-file",
        "--dry-run",
        "--inbox-dir",
        "--max-payload-bytes",
        "--message",
        "--receipt-dir",
        "--response-limit-bytes",
        "--timeout-secs",
        "--torii-base-url",
    },
    "notary": {
        "--all",
        "--allow-insecure-http",
        "--bearer-token-file",
        "--dry-run",
        "--endpoint",
        "--export-dir",
        "--receipt-dir",
        "--response-limit-bytes",
        "--timeout-secs",
    },
    "verify": {
        "--allow-failed",
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--receipt",
        "--receipt-dir",
        "--require-source-files",
    },
}
STAGE_SINGLETON_FLAGS = {
    "rail": set(EXPECTED_STAGE_FLAGS["rail"]),
    "notary": set(EXPECTED_STAGE_FLAGS["notary"]) - {"--endpoint"},
    "verify": {
        "--allow-failed",
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--require-source-files",
    },
}
STAGE_BOOLEAN_FLAGS = {
    "rail": {
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--dry-run",
    },
    "notary": {
        "--all",
        "--allow-insecure-http",
        "--dry-run",
    },
    "verify": {
        "--allow-failed",
        "--allow-default-profile",
        "--allow-insecure-http",
        "--allow-legacy-colr007",
        "--require-source-files",
    },
}
STAGE_POSITIVE_INT_FLAGS = {
    "rail": {
        "--max-payload-bytes",
        "--response-limit-bytes",
    },
    "notary": {
        "--response-limit-bytes",
    },
}
STAGE_POSITIVE_NUMBER_FLAGS = {
    "rail": {
        "--timeout-secs",
    },
    "notary": {
        "--timeout-secs",
    },
}
STAGE_ARTIFACT_PATH_FLAGS = {
    "rail": {
        "--inbox-dir",
    },
    "notary": {
        "--export-dir",
    },
    "verify": {
        "--receipt-dir",
    },
}
STAGE_XML_PATH_FLAGS = {
    "rail": {
        "--message",
    },
}
STAGE_RECEIPT_PATH_FLAGS = {
    "verify": {
        "--receipt",
    },
}
STAGE_REQUIRED_FLAGS = {
    "rail": {
        "--inbox-dir",
        "--torii-base-url",
    },
    "notary": {
        "--endpoint",
        "--export-dir",
    },
}
STAGE_REQUIRED_ONE_OF_FLAGS = {
    "verify": (
        ("--receipt", "--receipt-dir"),
    ),
}
LOCAL_DIAGNOSTIC_STAGE_FLAGS = {
    "notary": {"--allow-missing-record-sources"},
}
COMMAND_URL_FLAGS = {"--endpoint", "--torii-base-url"}
CANARY_SUMMARY_KEYS = {
    "version",
    "provider",
    "environment",
    "config_path",
    "started_at",
    "finished_at",
    "ok",
    "plan_only",
    "policy",
    "planned_stages",
    "stages",
    SUMMARY_DIGEST_FIELD,
}
CANARY_POLICY_KEYS = {"require_explicit_policy"}
CANARY_STAGE_KEYS = {
    "name",
    "started_at",
    "finished_at",
    "returncode",
    "command",
    "stdout_preview",
    "stderr_preview",
    "stdout_truncated",
    "stderr_truncated",
    "receipt_dir",
    "timed_out",
    "skipped",
    "reason",
}
CANARY_PLANNED_STAGE_KEYS = {"name", "command", "receipt_dir", "dry_run"}
RECEIPT_SUMMARY_KEYS = {
    "version",
    "verified_receipts",
    "receipt_kind",
    "allow_failed",
    "allow_insecure_http",
    "allow_legacy_colr007",
    "allow_default_profile",
    "require_source_files",
    "receipts",
    SUMMARY_DIGEST_FIELD,
}
RECEIPT_ENTRY_KEYS = {
    "path",
    "receipt_kind",
    "receipt_sha256",
    "ok",
    "status_code",
    "anchor_sha256",
    "index_sha256",
    "record_count",
    "message_type",
    "payload_sha256",
    "profile",
    "rail_message_id",
}
NOTARY_RECEIPT_METADATA_KEYS = {"anchor_sha256", "index_sha256", "record_count"}
RAIL_RECEIPT_METADATA_KEYS = {
    "message_type",
    "payload_sha256",
    "profile",
    "rail_message_id",
}
TRUST_SUMMARY_KEYS = {
    "version",
    "verified_at",
    "verified_bundles",
    "allow_record_only",
    "allow_insecure_source_url",
    "allow_synthetic_der",
    "max_source_age_days",
    "profile_json_emitted",
    "profile_json_emittable",
    "profile_json_sha256",
    "bundles",
    SUMMARY_DIGEST_FIELD,
}
TRUST_BUNDLE_KEYS = {
    "path",
    "profile_id",
    "rail",
    "environment",
    "source",
    "embedded_signature_policy",
    "material",
    "x509_trust_anchors",
    "revoked_certificates",
    "x509_crls",
    "x509_ocsp_responses",
    "profile_overrides",
    "bundle_sha256",
}
TRUST_SOURCE_KEYS = {"authority", "version", "url", "retrieved_at"}
TRUST_MATERIAL_KEYS = {
    "signature_public_key_pin_count",
    "x509_trust_anchor_pin_count",
    "revoked_certificate_pin_count",
    "x509_crl_count",
    "x509_ocsp_response_count",
    "x509_required_certificate_policy_oid_count",
}
TRUST_PROFILE_OVERRIDE_KEYS = {
    "id",
    "rail",
    "embedded_signature_policy",
    "signature_public_key_sha256_pins",
    "trusted_public_key_sha256",
    "x509_trust_anchor_sha256_pins",
    "trusted_certificate_sha256",
    "revoked_certificate_sha256",
    "x509_required_certificate_policy_oids",
    "x509_require_crl_revocation_check",
    "x509_crl_der_base64",
    "x509_require_ocsp_revocation_check",
    "x509_ocsp_response_der_base64",
}
TRUST_DER_SUMMARY_KEYS = {"label", "sha256", "byte_len"}

SECRET_KEY_FRAGMENTS = (
    "authorization",
    "private_key",
    "private-key",
    "password",
    "passphrase",
    "api_key",
    "api-key",
    "access_key",
    "access-key",
    "session_key",
    "session-key",
    "client_secret",
    "client-secret",
    "cookie",
    "set-cookie",
    "x-iroha-signature",
    "x_iroha_signature",
)
SECRET_KEY_EXACT = {
    "bearer",
    "bearer_token",
    "secret",
    "token",
}
SECRET_VALUE_PATTERNS = [
    re.compile(r"\bauthorization\s*:", re.IGNORECASE),
    re.compile(r"\bbearer\s+[A-Za-z0-9._~+/=-]+", re.IGNORECASE),
    re.compile(
        r"\b(?:token|secret|private[_-]?key|password|passphrase|api[_-]?key|access[_-]?key|session[_-]?key|client[_-]?secret|cookie|set-cookie)\s*[:=]\s*\S+",
        re.IGNORECASE,
    ),
    re.compile(r"\bx-iroha-signature\s*:", re.IGNORECASE),
]
SECRET_IDENTIFIER_PATTERN = re.compile(
    r"(?<![a-z0-9])"
    r"(?:authorization|bearer|token|secret|private[_-]?key|password|passphrase|"
    r"api[_-]?key|access[_-]?key|session[_-]?key|client[_-]?secret|cookie|"
    r"set-cookie|x[_-]iroha[_-]signature)"
    r"(?![a-z0-9])",
    re.IGNORECASE,
)


def _secret_scan_values(raw: str) -> tuple[str, ...]:
    values = [raw]
    decoded = raw
    for _ in range(4):
        if "%" not in decoded:
            break
        next_decoded = urllib.parse.unquote(decoded)
        if next_decoded == decoded:
            break
        values.append(next_decoded)
        decoded = next_decoded
    return tuple(values)


def _contains_secret_material(value: str) -> bool:
    return any(
        pattern.search(candidate)
        for candidate in _secret_scan_values(value)
        for pattern in SECRET_VALUE_PATTERNS
    )


def _contains_secret_identifier_material(value: str) -> bool:
    strong_markers = (
        "private_key",
        "private-key",
        "password",
        "passphrase",
        "api_key",
        "api-key",
        "access_key",
        "access-key",
        "session_key",
        "session-key",
        "client_secret",
        "client-secret",
        "set-cookie",
        "x-iroha-signature",
        "x_iroha_signature",
    )
    paired_markers = ("authorization", "bearer", "token", "cookie")
    return any(
        any(marker in lowered for marker in strong_markers)
        or ("secret" in lowered and any(marker in lowered for marker in paired_markers))
        for lowered in (candidate.lower() for candidate in _secret_scan_values(value))
    )


class EvidenceError(RuntimeError):
    """Raised when archived ISO operator evidence is unsafe or malformed."""


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _read_regular_file(path: Path, *, max_bytes: int | None = None) -> bytes:
    if max_bytes is not None and (
        isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or max_bytes <= 0
    ):
        raise EvidenceError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"{path} does not exist") from error
    mode = metadata.st_mode
    if stat.S_ISLNK(mode):
        raise EvidenceError(f"{path} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise EvidenceError(f"{path} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise EvidenceError(f"{path} exceeds {max_bytes} byte JSON limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise EvidenceError(f"{path} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise EvidenceError(f"{path} exceeds {max_bytes} byte JSON limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise EvidenceError(f"{path} exceeds {max_bytes} byte JSON limit")
        return raw
    except FileNotFoundError as error:
        raise EvidenceError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise EvidenceError(f"{path} must not be a symlink") from error
        raise EvidenceError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise EvidenceError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise EvidenceError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise EvidenceError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise EvidenceError(f"{label} must use forward slashes")
    if ";" in raw:
        raise EvidenceError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise EvidenceError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise EvidenceError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise EvidenceError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise EvidenceError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise EvidenceError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise EvidenceError(f"{label} must use forward slashes")
    if ";" in raw:
        raise EvidenceError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise EvidenceError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise EvidenceError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise EvidenceError(f"{label} must not contain dot or parent segments")


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise EvidenceError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise EvidenceError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise EvidenceError(f"{arg} does not take a value")
        index += 1


def _preflight_required_cli_values(
    argv: list[str] | None,
    flags: set[str],
    value_name: str,
) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            return
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _preflight_output_cli_paths(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            return
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise EvidenceError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{flag} must be a numeric value")
    try:
        int(raw, 10) if integer else float(raw)
    except ValueError as error:
        raise EvidenceError(f"{flag} must be a numeric value") from error


def _preflight_numeric_cli_values(
    argv: list[str] | None,
    *,
    integer_flags: set[str],
    number_flags: set[str],
) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    flags = integer_flags | number_flags
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            return
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise EvidenceError(f"{flag} requires a numeric value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _write_text_output(path: Path, text: str) -> None:
    _reject_output_path_smuggling(path, "output path")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except FileExistsError as error:
        raise EvidenceError(f"{path.parent} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise EvidenceError(f"{path.parent} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise EvidenceError(f"{path.parent} must be a directory")
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise EvidenceError(f"{path} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise EvidenceError(f"{path} must be a regular file")
        if metadata.st_nlink > 1:
            raise EvidenceError(f"{path} must not be hard-linked")
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise EvidenceError(f"{path.parent} must not be a symlink") from error
        raise EvidenceError(f"{path.parent} must be a directory") from error

    fd = -1
    leaf_digest = hashlib.sha256(path.name.encode("utf-8", "surrogatepass")).hexdigest()
    tmp_name = f".iso-{leaf_digest[:16]}.{os.getpid()}.{secrets.token_hex(8)}.tmp"
    tmp_created = False
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_CLOEXEC", 0)
        try:
            fd = os.open(tmp_name, flags | nofollow, 0o600, dir_fd=parent_fd)
            tmp_created = True
        except OSError as error:
            if error.errno == errno.ELOOP:
                raise EvidenceError(f"{path} temp file must not be a symlink") from error
            raise EvidenceError(
                f"cannot open temporary output for {path}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise EvidenceError(f"{path} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise EvidenceError(f"{path} temp file must not be hard-linked")
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(tmp_name, path.name, src_dir_fd=parent_fd, dst_dir_fd=parent_fd)
        tmp_created = False
        try:
            os.fsync(parent_fd)
        except OSError:
            pass
    finally:
        if fd >= 0:
            os.close(fd)
        if tmp_created:
            try:
                os.unlink(tmp_name, dir_fd=parent_fd)
            except FileNotFoundError:
                pass
        os.close(parent_fd)


def _reject_symlinked_existing_ancestors(path: Path) -> None:
    current = Path(path.anchor) if path.is_absolute() else Path(".")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    for part in parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            return
        if stat.S_ISLNK(mode):
            if path.is_absolute() and current.parent == Path(path.anchor):
                continue
            raise EvidenceError(f"{current} must not be a symlink")


def _load_json(path: Path) -> Any:
    try:
        raw = _read_regular_file(path, max_bytes=MAX_SUMMARY_JSON_BYTES)
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{path} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{path} is not valid JSON: {error}") from error
    _reject_json_surrogates(value)
    return value


def _read_limited_pipe(pipe: Any, limit_bytes: int) -> tuple[bytes, bool]:
    chunks: list[bytes] = []
    remaining = limit_bytes
    truncated = False
    while True:
        chunk = pipe.read(8192)
        if not chunk:
            break
        if remaining > 0:
            keep = min(remaining, len(chunk))
            chunks.append(chunk[:keep])
            remaining -= keep
            if keep < len(chunk):
                truncated = True
        else:
            truncated = True
    return b"".join(chunks), truncated


def _run_command_bounded(
    argv: list[str],
    output_limit_bytes: int,
    timeout_secs: float,
) -> tuple[int, str, bool, str, bool, bool]:
    if (
        isinstance(output_limit_bytes, bool)
        or not isinstance(output_limit_bytes, int)
        or output_limit_bytes <= 0
    ):
        raise EvidenceError("output limit bytes must be positive")
    timeout_secs = _required_positive_finite_cli_number(
        timeout_secs,
        "receipt verifier timeout seconds",
    )
    process = subprocess.Popen(
        argv,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    outputs: dict[str, tuple[bytes, bool]] = {}

    def read_stream(name: str, pipe: Any) -> None:
        try:
            outputs[name] = _read_limited_pipe(pipe, output_limit_bytes)
        finally:
            pipe.close()

    assert process.stdout is not None
    assert process.stderr is not None
    stdout_thread = threading.Thread(
        target=read_stream,
        args=("stdout", process.stdout),
        daemon=True,
    )
    stderr_thread = threading.Thread(
        target=read_stream,
        args=("stderr", process.stderr),
        daemon=True,
    )
    stdout_thread.start()
    stderr_thread.start()
    timed_out = False
    try:
        returncode = process.wait(timeout=timeout_secs)
    except subprocess.TimeoutExpired:
        timed_out = True
        process.kill()
        process.wait()
        returncode = 124
    stdout_thread.join()
    stderr_thread.join()
    stdout_raw, stdout_truncated = outputs.get("stdout", (b"", False))
    stderr_raw, stderr_truncated = outputs.get("stderr", (b"", False))
    return (
        returncode,
        stdout_raw.decode("utf-8", errors="replace"),
        stdout_truncated,
        stderr_raw.decode("utf-8", errors="replace"),
        stderr_truncated,
        timed_out,
    )


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError("duplicate key in JSON object")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise EvidenceError(f"JSON contains non-finite numeric constant {value}")


def _reject_json_surrogates(value: Any) -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise EvidenceError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        for item in value:
            _reject_json_surrogates(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            _reject_json_surrogates(key)
            _reject_json_surrogates(item)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        if any(_is_secret_looking_key(key) for key in unknown):
            raise EvidenceError(f"{label} contains unknown keys")
        raise EvidenceError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _is_secret_looking_key(value: Any) -> bool:
    return any(
        SECRET_IDENTIFIER_PATTERN.search(candidate)
        for candidate in _secret_scan_values(str(value))
    )


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise EvidenceError(f"{label} must not contain secret-looking material")


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise EvidenceError(f"{label} must be a JSON array")
    return value


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise EvidenceError(f"{label}.{key} must be a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{label}.{key} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _required_positive_int_field(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise EvidenceError(f"{label}.{key} must be a positive integer")
    return raw


def _required_profile_id(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    if PROFILE_ID_RE.fullmatch(raw) is None:
        raise EvidenceError(f"{label}.{key} must be a canonical lowercase profile id")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_rail(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    if raw not in KNOWN_RAILS:
        raise EvidenceError(
            f"{label}.{key} must be one of " + ", ".join(sorted(KNOWN_RAILS))
        )
    return raw


def _required_message_type(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    if MESSAGE_TYPE_RE.fullmatch(raw) is None:
        raise EvidenceError(f"{label}.{key} must be lowercase ISO family id")
    return raw


def _nullable_rail_message_id(value: dict[str, Any], key: str, label: str) -> str | None:
    if key not in value:
        raise EvidenceError(f"{label}.{key} must be recorded")
    raw = value[key]
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise EvidenceError(f"{label}.{key} must be null or a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{label}.{key} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{label}.{key} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise EvidenceError(f"{label}.{key} must not contain whitespace")
    if len(raw) > MAX_RAIL_MESSAGE_ID_CHARS:
        raise EvidenceError(
            f"{label}.{key} must be at most {MAX_RAIL_MESSAGE_ID_CHARS} characters"
        )
    if RAIL_MESSAGE_ID_RE.fullmatch(raw) is None:
        raise EvidenceError(f"{label}.{key} must be a canonical ASCII rail message id")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_receipt_kind(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_stage_name(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _reject_forbidden_receipt_metadata(
    receipt_entry: dict[str, Any],
    forbidden_keys: set[str],
    entry_label: str,
    receipt_kind: str,
) -> None:
    for key in sorted(forbidden_keys & set(receipt_entry)):
        raise EvidenceError(f"{entry_label}.{key} is not valid for {receipt_kind}")


def _verify_receipt_entry_metadata(
    receipt_entry: dict[str, Any],
    entry_label: str,
    *,
    receipt_kind: str,
    allow_legacy_colr007: bool,
    allow_default_profile: bool,
) -> None:
    if receipt_kind == "iso-audit-notary":
        _reject_forbidden_receipt_metadata(
            receipt_entry,
            RAIL_RECEIPT_METADATA_KEYS,
            entry_label,
            receipt_kind,
        )
        _required_sha256(receipt_entry, "anchor_sha256", entry_label)
        _required_sha256(receipt_entry, "index_sha256", entry_label)
        record_count = receipt_entry.get("record_count")
        if (
            isinstance(record_count, bool)
            or not isinstance(record_count, int)
            or record_count <= 0
        ):
            raise EvidenceError(f"{entry_label}.record_count must be a positive integer")
    elif receipt_kind == "iso-rail-gateway":
        _reject_forbidden_receipt_metadata(
            receipt_entry,
            NOTARY_RECEIPT_METADATA_KEYS,
            entry_label,
            receipt_kind,
        )
        message_type = _required_message_type(receipt_entry, "message_type", entry_label)
        if message_type not in SUPPORTED_RAIL_MESSAGE_TYPES:
            raise EvidenceError(
                f"{entry_label}.message_type is unsupported: {message_type!r}"
            )
        if message_type in LEGACY_RAIL_MESSAGE_TYPES and not allow_legacy_colr007:
            raise EvidenceError(
                f"{entry_label}.message_type uses legacy rail message type {message_type!r}"
            )
        _required_sha256(receipt_entry, "payload_sha256", entry_label)
        if "profile" not in receipt_entry:
            raise EvidenceError(f"{entry_label}.profile must be recorded")
        if receipt_entry["profile"] is None:
            if not allow_default_profile:
                raise EvidenceError(f"{entry_label}.profile must be a non-empty string")
        else:
            _required_profile_id(receipt_entry, "profile", entry_label)
        _nullable_rail_message_id(receipt_entry, "rail_message_id", entry_label)
    else:  # pragma: no cover - supported kinds are checked before this helper.
        raise EvidenceError(f"{entry_label}.receipt_kind is unsupported: {receipt_kind!r}")


def _receipt_entry_content_metadata(receipt_entry: dict[str, Any]) -> tuple[tuple[str, Any], ...]:
    receipt_kind = receipt_entry["receipt_kind"]
    generic_keys = ("ok", "status_code")
    if receipt_kind == "iso-audit-notary":
        keys = ("anchor_sha256", "index_sha256", "record_count")
    elif receipt_kind == "iso-rail-gateway":
        keys = ("message_type", "payload_sha256", "profile", "rail_message_id")
    else:  # pragma: no cover - supported kinds are checked before this helper.
        raise EvidenceError(f"unsupported receipt_kind {receipt_kind!r}")
    return tuple((key, receipt_entry.get(key)) for key in (*generic_keys, *keys))


def _required_cli_string(value: str | None, label: str) -> str:
    if value is None or not value.strip():
        raise EvidenceError(f"provide {label}")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise EvidenceError(f"{label} must not contain control characters")
    if value != value.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    _reject_secret_looking_identifier(value, label)
    return value


def _required_positive_cli_int(value: int | None, label: str) -> int:
    if value is None:
        raise EvidenceError(f"provide {label}")
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise EvidenceError(f"{label} must be a positive integer")
    return value


def _required_positive_finite_cli_number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise EvidenceError(f"{label} must be a positive finite number")
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise EvidenceError(f"{label} must be a positive finite number")
    return parsed


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path)
        if key in seen:
            raise EvidenceError(f"{label}[{offset}] duplicates {label}[{seen[key]}]")
        seen[key] = offset


def _reject_duplicate_summary_digests(summaries: list[dict[str, Any]], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, summary in enumerate(summaries):
        digest = summary[SUMMARY_DIGEST_FIELD]
        if digest in seen:
            raise EvidenceError(
                f"{label}[{offset}].{SUMMARY_DIGEST_FIELD} duplicates "
                f"{label}[{seen[digest]}].{SUMMARY_DIGEST_FIELD}"
            )
        seen[digest] = offset


def _required_timestamp(value: dict[str, Any], key: str, label: str) -> tuple[str, dt.datetime]:
    raw = _required_string(value, key, label)
    return raw, _parse_timestamp(raw, f"{label}.{key}")


def _reject_stale_timestamp(
    timestamp: dt.datetime,
    *,
    max_age_days: int,
    label: str,
) -> None:
    cutoff = dt.datetime.now(dt.UTC) - dt.timedelta(days=max_age_days)
    if timestamp < cutoff:
        raise EvidenceError(f"{label} is older than the {max_age_days}-day freshness budget")


def _required_bool(value: dict[str, Any], key: str, label: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise EvidenceError(f"{label}.{key} must be a boolean")
    return raw


def _required_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
        raise EvidenceError(f"{label}.{key} must be a non-negative integer")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _required_sha256(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if isinstance(raw, str):
        _reject_secret_looking_identifier(raw, f"{label}.{key}")
    if not _is_lower_sha256(raw):
        raise EvidenceError(f"{label}.{key} must be a lowercase SHA-256 digest")
    return raw


def _required_sha256_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _require_list(value.get(key), f"{label}.{key}")
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, item in enumerate(items):
        if isinstance(item, str):
            _reject_secret_looking_identifier(item, f"{label}.{key}[{offset}]")
        if not _is_lower_sha256(item):
            raise EvidenceError(f"{label}.{key}[{offset}] must be a canonical SHA-256")
        if item in seen:
            raise EvidenceError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[item]}]"
            )
        seen[item] = offset
        result.append(item)
    return result


def _required_clean_string_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _require_list(value.get(key), f"{label}.{key}")
    result: list[str] = []
    for offset, item in enumerate(items):
        if not isinstance(item, str) or not item.strip():
            raise EvidenceError(f"{label}.{key}[{offset}] must be a non-empty string")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise EvidenceError(f"{label}.{key}[{offset}] must not contain control characters")
        if item != item.strip():
            raise EvidenceError(f"{label}.{key}[{offset}] must not have surrounding whitespace")
        result.append(item)
    return result


def _valid_oid(value: str) -> bool:
    parts = value.split(".")
    if len(parts) < 2:
        return False
    for part in parts:
        if not part or not part.isascii() or not part.isdecimal():
            return False
        if len(part) > 1 and part.startswith("0"):
            return False
    first = int(parts[0])
    if first > 2:
        return False
    if first < 2 and int(parts[1]) > 39:
        return False
    return True


def _required_oid_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _required_clean_string_list(value, key, label)
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, item in enumerate(items):
        _reject_secret_looking_identifier(item, f"{label}.{key}[{offset}]")
        if not _valid_oid(item):
            raise EvidenceError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
        if item in seen:
            raise EvidenceError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[item]}]"
            )
        seen[item] = offset
        result.append(item)
    return result


def _required_canonical_base64_list(
    value: dict[str, Any],
    key: str,
    label: str,
) -> list[str]:
    items = _required_clean_string_list(value, key, label)
    if len(items) > MAX_TRUST_DER_BLOBS:
        raise EvidenceError(
            f"{label}.{key} must not contain more than {MAX_TRUST_DER_BLOBS} entries"
        )
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, item in enumerate(items):
        if len(item) > MAX_TRUST_DER_BASE64_CHARS:
            raise EvidenceError(
                f"{label}.{key}[{offset}] must decode to no more than "
                f"{MAX_TRUST_DER_BYTES} bytes"
            )
        try:
            decoded = base64.b64decode(item, validate=True)
        except (ValueError, binascii.Error) as error:
            raise EvidenceError(f"{label}.{key}[{offset}] must be canonical base64") from error
        if not decoded:
            raise EvidenceError(f"{label}.{key}[{offset}] must be non-empty base64")
        if len(decoded) > MAX_TRUST_DER_BYTES:
            raise EvidenceError(
                f"{label}.{key}[{offset}] must decode to no more than "
                f"{MAX_TRUST_DER_BYTES} bytes"
            )
        _require_der_sequence(decoded, f"{label}.{key}[{offset}]")
        canonical = base64.b64encode(decoded).decode("ascii")
        if canonical != item:
            raise EvidenceError(f"{label}.{key}[{offset}] must be canonical padded base64")
        if canonical in seen:
            raise EvidenceError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[canonical]}]"
            )
        seen[canonical] = offset
        result.append(canonical)
    return result


def _require_der_sequence(value: bytes, label: str) -> None:
    if not value or value[0] != 0x30:
        raise EvidenceError(f"{label} must be a DER SEQUENCE")
    if len(value) < 2:
        raise EvidenceError(f"{label} has truncated DER length")

    first_length = value[1]
    header_len = 2
    if first_length < 0x80:
        content_len = first_length
    else:
        length_octets = first_length & 0x7F
        if length_octets == 0 or length_octets > 4:
            raise EvidenceError(f"{label} has invalid DER length")
        if len(value) < 2 + length_octets:
            raise EvidenceError(f"{label} has truncated DER length")
        length_bytes = value[2 : 2 + length_octets]
        if length_bytes[0] == 0:
            raise EvidenceError(f"{label} has non-minimal DER length")
        content_len = int.from_bytes(length_bytes, "big")
        if content_len < 0x80:
            raise EvidenceError(f"{label} has non-minimal DER length")
        header_len += length_octets

    if header_len + content_len != len(value):
        raise EvidenceError(
            f"{label} DER length does not consume the whole value"
        )


def _required_der_summary_entries(
    bundle: dict[str, Any],
    key: str,
    label: str,
) -> dict[str, int]:
    items = _require_list(bundle.get(key), f"{label}.{key}")
    if len(items) > MAX_TRUST_DER_BLOBS:
        raise EvidenceError(
            f"{label}.{key} must not contain more than {MAX_TRUST_DER_BLOBS} entries"
        )
    result: dict[str, int] = {}
    seen_labels: dict[str, int] = {}
    for offset, raw_entry in enumerate(items):
        entry_label = f"{label}.{key}[{offset}]"
        entry = _require_object(raw_entry, entry_label)
        _reject_unknown_keys(entry, TRUST_DER_SUMMARY_KEYS, entry_label)
        if "label" in entry:
            raw_label = entry.get("label")
            if not isinstance(raw_label, str) or not raw_label.strip():
                raise EvidenceError(
                    f"{entry_label}.label must be a non-empty string when provided"
                )
            if raw_label != raw_label.strip():
                raise EvidenceError(f"{entry_label}.label must not have surrounding whitespace")
            if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw_label):
                raise EvidenceError(f"{entry_label}.label must not contain control characters")
            if len(raw_label) > 128:
                raise EvidenceError(f"{entry_label}.label must be no longer than 128 characters")
            _reject_secret_looking_identifier(raw_label, f"{entry_label}.label")
            if raw_label in seen_labels:
                raise EvidenceError(
                    f"{entry_label}.label duplicates {label}.{key}[{seen_labels[raw_label]}].label"
                )
            seen_labels[raw_label] = offset
        digest = entry.get("sha256")
        if not _is_lower_sha256(digest):
            raise EvidenceError(f"{entry_label}.sha256 must be a canonical SHA-256")
        if digest in result:
            raise EvidenceError(f"{entry_label}.sha256 duplicates DER SHA-256")
        byte_len = _required_nonnegative_int(entry, "byte_len", entry_label)
        if byte_len == 0 or byte_len > MAX_TRUST_DER_BYTES:
            raise EvidenceError(
                f"{entry_label}.byte_len must be positive and no more than "
                f"{MAX_TRUST_DER_BYTES}"
            )
        result[digest] = byte_len
    return result


def _canonical_base64_der_entries(values: list[str]) -> dict[str, int]:
    result: dict[str, int] = {}
    for item in values:
        der = base64.b64decode(item, validate=True)
        result[sha256_hex(der)] = len(der)
    return result


def _require_summary_digests_in_pins(
    entries: dict[str, int],
    pins: list[str],
    summary_label: str,
    pins_label: str,
) -> None:
    missing = sorted(set(entries) - set(pins))
    if missing:
        raise EvidenceError(
            f"{summary_label} contains DER SHA-256 {missing[0]} missing from {pins_label}"
        )


def _require_override_der_matches_summary(
    values: list[str],
    entries: dict[str, int],
    override_label: str,
    summary_label: str,
) -> None:
    override_entries = _canonical_base64_der_entries(values)
    extra = sorted(set(override_entries) - set(entries))
    if extra:
        raise EvidenceError(
            f"{override_label} contains DER SHA-256 {extra[0]} not recorded in {summary_label}"
        )
    missing = sorted(set(entries) - set(override_entries))
    if missing:
        raise EvidenceError(
            f"{summary_label} contains DER SHA-256 {missing[0]} missing from {override_label}"
        )
    for digest, byte_len in override_entries.items():
        if entries[digest] != byte_len:
            raise EvidenceError(
                f"{summary_label} byte_len does not match {override_label} "
                f"for DER SHA-256 {digest}"
            )


def _compact_der_entries(entries: dict[str, int]) -> list[dict[str, int | str]]:
    return [
        {
            "sha256": digest,
            "byte_len": entries[digest],
        }
        for digest in sorted(entries)
    ]


def _reject_sha256_overlap(first: list[str], second: list[str], label: str) -> None:
    overlap = sorted(set(first) & set(second))
    if overlap:
        raise EvidenceError(f"{label} contains overlapping SHA-256 pin {overlap[0]}")


def _validate_receipt_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    if not raw.endswith(RECEIPT_PATH_SUFFIX):
        raise EvidenceError(f"{label} must point to a {RECEIPT_PATH_SUFFIX} file")
    return raw


def _validate_config_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".json"):
        raise EvidenceError(f"{label} must point to a .json file")
    return raw


def _validate_xml_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".xml"):
        raise EvidenceError(f"{label} must point to a .xml file")
    return raw


def _validate_artifact_path(raw: str, label: str) -> str:
    if not raw.strip():
        raise EvidenceError(f"{label} must be a non-empty path")
    _reject_path_smuggling(raw, label)
    return raw


def _reject_path_smuggling(raw: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise EvidenceError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise EvidenceError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise EvidenceError(f"{label} must not start with a dash")
    if ";" in raw:
        raise EvidenceError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise EvidenceError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise EvidenceError(f"{label} must not contain leading-dash path segments")
    normalized_parts = [part for part in raw.replace("\\", "/").split("/") if part]
    if any(part in {".", ".."} for part in normalized_parts):
        raise EvidenceError(f"{label} must not contain dot or parent segments")
    if "\\" in raw:
        raise EvidenceError(f"{label} must use forward slashes")


def _require_summary_digest(summary: dict[str, Any], label: str) -> str:
    expected = summary.get(SUMMARY_DIGEST_FIELD)
    if not _is_lower_sha256(expected):
        raise EvidenceError(f"{label} has missing or non-canonical {SUMMARY_DIGEST_FIELD}")
    body = dict(summary)
    body.pop(SUMMARY_DIGEST_FIELD)
    actual = sha256_hex(_canonical_json_bytes(body))
    if actual != expected:
        raise EvidenceError(
            f"{label} {SUMMARY_DIGEST_FIELD} mismatch: expected {expected}, got {actual}"
        )
    return expected


def _verify_receipt_verifier_summary(
    receipt_obj: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    digest = _require_summary_digest(receipt_obj, label)
    _reject_unknown_keys(receipt_obj, RECEIPT_SUMMARY_KEYS, label)
    _check_no_secret_material(receipt_obj, label)
    version = receipt_obj.get("version")
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version != RECEIPT_SUMMARY_VERSION
    ):
        raise EvidenceError(
            f"{label}.version must be receipt verifier summary version "
            f"{RECEIPT_SUMMARY_VERSION}"
        )
    verified_receipts = receipt_obj.get("verified_receipts")
    if (
        isinstance(verified_receipts, bool)
        or not isinstance(verified_receipts, int)
        or verified_receipts <= 0
    ):
        raise EvidenceError(f"{label}.verified_receipts must be positive")
    allow_failed = _required_bool(receipt_obj, "allow_failed", label)
    if allow_failed and not args.allow_failed_receipts:
        raise EvidenceError(f"{label} allowed failed receipts")
    allow_insecure_http = _required_bool(receipt_obj, "allow_insecure_http", label)
    if allow_insecure_http and not args.allow_insecure_http:
        raise EvidenceError(f"{label} allowed insecure HTTP receipts")
    allow_legacy_colr007 = _required_bool(receipt_obj, "allow_legacy_colr007", label)
    if allow_legacy_colr007 and not args.allow_legacy_colr007:
        raise EvidenceError(f"{label} allowed legacy colr.007 receipts")
    allow_default_profile = _required_bool(receipt_obj, "allow_default_profile", label)
    if allow_default_profile and not args.allow_default_profile:
        raise EvidenceError(f"{label} allowed default rail profile fallback")
    require_source_files = _required_bool(
        receipt_obj,
        "require_source_files",
        label,
    )
    if not require_source_files and not args.allow_receipt_source_missing:
        raise EvidenceError(f"{label} did not require receipt source files")

    receipt_kind = _require_list(
        receipt_obj.get("receipt_kind"),
        f"{label}.receipt_kind",
    )
    if not receipt_kind:
        raise EvidenceError(f"{label}.receipt_kind must contain strings")
    seen_receipt_kinds: dict[str, int] = {}
    for offset, item in enumerate(receipt_kind):
        if not isinstance(item, str) or not item.strip():
            raise EvidenceError(f"{label}.receipt_kind must contain strings")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise EvidenceError(
                f"{label}.receipt_kind[{offset}] must not contain control characters"
            )
        if item != item.strip():
            raise EvidenceError(
                f"{label}.receipt_kind[{offset}] must not have surrounding whitespace"
            )
        _reject_secret_looking_identifier(item, f"{label}.receipt_kind[{offset}]")
        if item in seen_receipt_kinds:
            raise EvidenceError(
                f"{label}.receipt_kind[{offset}] duplicates "
                f"{label}.receipt_kind[{seen_receipt_kinds[item]}]"
            )
        seen_receipt_kinds[item] = offset
    receipt_kind_set = set(receipt_kind)
    if args.allow_partial_canary:
        if not (receipt_kind_set & REQUIRED_RECEIPT_KINDS):
            raise EvidenceError(f"{label} has no rail/notary receipt kinds")
    else:
        missing = sorted(REQUIRED_RECEIPT_KINDS - receipt_kind_set)
        if missing:
            raise EvidenceError(f"{label} is missing receipt kinds: {', '.join(missing)}")

    receipt_entries_raw = _require_list(receipt_obj.get("receipts"), f"{label}.receipts")
    if len(receipt_entries_raw) != verified_receipts:
        raise EvidenceError(f"{label}.receipts length does not match verified_receipts")
    receipt_entries: list[dict[str, Any]] = []
    receipt_entry_kinds: set[str] = set()
    seen_receipt_paths: dict[str, int] = {}
    seen_receipt_digests: dict[str, int] = {}
    for offset, receipt_entry_raw in enumerate(receipt_entries_raw):
        entry_label = f"{label}.receipts[{offset}]"
        receipt_entry = _require_object(receipt_entry_raw, entry_label)
        _reject_unknown_keys(receipt_entry, RECEIPT_ENTRY_KEYS, entry_label)
        receipt_path = _validate_receipt_path(
            _required_string(receipt_entry, "path", entry_label),
            f"{entry_label}.path",
        )
        if receipt_path in seen_receipt_paths:
            raise EvidenceError(
                f"{entry_label}.path duplicates "
                f"{label}.receipts[{seen_receipt_paths[receipt_path]}].path"
            )
        seen_receipt_paths[receipt_path] = offset
        entry_kind = _required_receipt_kind(receipt_entry, "receipt_kind", entry_label)
        if entry_kind not in REQUIRED_RECEIPT_KINDS:
            raise EvidenceError(f"{entry_label}.receipt_kind is unsupported: {entry_kind!r}")
        receipt_sha256 = receipt_entry.get("receipt_sha256")
        if not _is_lower_sha256(receipt_sha256):
            raise EvidenceError(f"{entry_label}.receipt_sha256 must be a canonical SHA-256")
        if receipt_sha256 in seen_receipt_digests:
            raise EvidenceError(
                f"{entry_label}.receipt_sha256 duplicates "
                f"{label}.receipts[{seen_receipt_digests[receipt_sha256]}].receipt_sha256"
            )
        seen_receipt_digests[receipt_sha256] = offset
        ok = receipt_entry.get("ok")
        if not isinstance(ok, bool):
            raise EvidenceError(f"{entry_label}.ok must be a boolean")
        status_code = receipt_entry.get("status_code")
        if (
            isinstance(status_code, bool)
            or not isinstance(status_code, int)
            or status_code < 100
        ):
            raise EvidenceError(f"{entry_label}.status_code must be an HTTP status integer")
        status_success = 200 <= status_code <= 299
        if ok != status_success:
            raise EvidenceError(f"{entry_label}.ok does not match status_code success state")
        if not ok:
            raise EvidenceError(f"{entry_label} did not succeed")
        _verify_receipt_entry_metadata(
            receipt_entry,
            entry_label,
            receipt_kind=entry_kind,
            allow_legacy_colr007=args.allow_legacy_colr007,
            allow_default_profile=allow_default_profile,
        )
        receipt_entry_kinds.add(entry_kind)
        receipt_entries.append(dict(receipt_entry))
    if receipt_kind_set != receipt_entry_kinds:
        raise EvidenceError(f"{label}.receipt_kind does not match receipts[].receipt_kind")

    return {
        "version": version,
        "verified_receipts": verified_receipts,
        "receipt_kind": sorted(receipt_kind_set),
        "allow_failed": allow_failed,
        "allow_insecure_http": allow_insecure_http,
        "allow_legacy_colr007": allow_legacy_colr007,
        "allow_default_profile": allow_default_profile,
        "require_source_files": require_source_files,
        "receipts": receipt_entries,
        "summary_sha256": digest,
    }


def _reject_secret_string(value: str, label: str) -> None:
    if _contains_secret_material(value):
        raise EvidenceError(f"{label} contains secret-looking material")


def _check_no_secret_material(value: Any, label: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if _is_secret_looking_key(key):
                raise EvidenceError(f"{label} contains forbidden secret-looking field")
            _check_no_secret_material(child, f"{label}.{key}")
    elif isinstance(value, list):
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{label}[{offset}]")
    elif isinstance(value, str):
        _reject_secret_string(value, label)


def _command_has_script(command: list[str], script_name: str) -> bool:
    return any(Path(item).name == script_name for item in command)


def _command_has_flag(command: list[str], flag: str) -> bool:
    return any(item == flag or item.startswith(flag + "=") for item in command)


def _command_flag_count(command: list[str], flag: str) -> int:
    prefix = flag + "="
    return sum(1 for item in command if item == flag or item.startswith(prefix))


def _command_separate_value_offsets(
    command: list[str],
    flags: set[str],
    label: str,
) -> set[int]:
    offsets: set[int] = set()
    for offset, item in enumerate(command):
        if item in flags:
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label}.command has {item} without a value")
            if command[offset + 1].startswith("--"):
                raise EvidenceError(f"{label}.command has {item} without a value")
            offsets.add(offset + 1)
    return offsets


def _command_flag_values(
    command: list[str],
    flag: str,
    label: str,
) -> list[tuple[int, str]]:
    values: list[tuple[int, str]] = []
    prefix = flag + "="
    for offset, item in enumerate(command):
        if item == flag:
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label}.command has {flag} without a value")
            if command[offset + 1].startswith("--"):
                raise EvidenceError(f"{label}.command has {flag} without a value")
            values.append((offset + 1, command[offset + 1]))
        elif item.startswith(prefix):
            value = item[len(prefix):]
            if not value or value.startswith("--"):
                raise EvidenceError(f"{label}.command has {flag} without a value")
            values.append((offset, value))
    return values


def _check_numeric_command_flags(stage_name: str, command: list[str], label: str) -> None:
    for flag in sorted(STAGE_POSITIVE_INT_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            if re.fullmatch(r"[1-9][0-9]*", value) is None:
                raise EvidenceError(
                    f"{label}.command[{offset}] {flag} must be a positive decimal integer"
                )
    for flag in sorted(STAGE_POSITIVE_NUMBER_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            try:
                parsed = float(value)
            except ValueError as error:
                raise EvidenceError(
                    f"{label}.command[{offset}] {flag} must be a positive finite number"
                ) from error
            if not math.isfinite(parsed) or parsed <= 0:
                raise EvidenceError(
                    f"{label}.command[{offset}] {flag} must be a positive finite number"
                )


def _check_path_command_flags(stage_name: str, command: list[str], label: str) -> None:
    for flag in sorted(STAGE_ARTIFACT_PATH_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            _validate_artifact_path(value, f"{label}.command[{offset}]")
    for flag in sorted(STAGE_XML_PATH_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            _validate_xml_path(value, f"{label}.command[{offset}]")
    for flag in sorted(STAGE_RECEIPT_PATH_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            _validate_receipt_path(value, f"{label}.command[{offset}]")


def _check_required_command_flags(stage_name: str, command: list[str], label: str) -> None:
    for flag in sorted(STAGE_REQUIRED_FLAGS.get(stage_name, set())):
        if _command_flag_count(command, flag) == 0:
            raise EvidenceError(f"{label}.command must contain {flag}")
    for choices in STAGE_REQUIRED_ONE_OF_FLAGS.get(stage_name, ()):
        if not any(_command_flag_count(command, flag) > 0 for flag in choices):
            raise EvidenceError(
                f"{label}.command must contain one of {', '.join(choices)}"
            )


def _command_has_http_url(command: list[str]) -> bool:
    return any(item.startswith("http://") for item in command)


def _check_command_urls(
    command: list[str],
    label: str,
    *,
    allow_insecure_http: bool,
) -> None:
    for offset, item in enumerate(command):
        if item in COMMAND_URL_FLAGS:
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label}.command has {item} without a value")
            if command[offset + 1].startswith("--"):
                raise EvidenceError(f"{label}.command has {item} without a value")
            _check_clean_http_url(
                command[offset + 1],
                f"{label}.command[{offset + 1}]",
                allow_insecure_http=allow_insecure_http,
                reject_local_hosts=True,
            )
            continue
        for flag in COMMAND_URL_FLAGS:
            prefix = flag + "="
            if item.startswith(prefix):
                value = item[len(prefix):]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{label}.command has {flag} without a value")
                _check_clean_http_url(
                    value,
                    f"{label}.command[{offset}]",
                    allow_insecure_http=allow_insecure_http,
                    reject_local_hosts=True,
                )
                break
        else:
            if item.startswith(("http://", "https://")):
                _check_clean_http_url(
                    item,
                    f"{label}.command[{offset}]",
                    allow_insecure_http=allow_insecure_http,
                    reject_local_hosts=True,
                )


def _check_redacted_bearer_files(command: list[str], label: str) -> None:
    for offset, item in enumerate(command):
        if item == "--bearer-token-file":
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if command[offset + 1].startswith("--"):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if command[offset + 1] != "<runtime-token-file>":
                raise EvidenceError(f"{label} contains an unredacted bearer-token file path")
            continue
        prefix = "--bearer-token-file="
        if item.startswith(prefix):
            value = item[len(prefix) :]
            if not value or value.startswith("--"):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if value != "<runtime-token-file>":
                raise EvidenceError(f"{label} contains an unredacted bearer-token file path")


def _check_command_policy(
    command: list[str],
    label: str,
    *,
    allow_dry_run: bool,
    allow_insecure_http: bool,
    allow_default_profile: bool,
    allow_failed_receipts: bool,
    allow_legacy_colr007: bool,
) -> None:
    if not command:
        raise EvidenceError(f"{label}.command must not be empty")
    if not all(isinstance(item, str) and item for item in command):
        raise EvidenceError(f"{label}.command must contain non-empty strings")
    for offset, item in enumerate(command):
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise EvidenceError(
                f"{label}.command[{offset}] must not contain control characters"
            )
        if item != item.strip():
            raise EvidenceError(
                f"{label}.command[{offset}] must not have surrounding whitespace"
            )
    _check_redacted_bearer_files(command, label)
    if _command_has_flag(command, "--dry-run") and not allow_dry_run:
        raise EvidenceError(f"{label} used --dry-run")
    if (
        _command_has_flag(command, "--allow-insecure-http") or _command_has_http_url(command)
    ) and not allow_insecure_http:
        raise EvidenceError(f"{label} used insecure HTTP evidence")
    _check_command_urls(command, label, allow_insecure_http=allow_insecure_http)
    if _command_has_flag(command, "--allow-default-profile") and not allow_default_profile:
        raise EvidenceError(f"{label} used --allow-default-profile")
    if _command_has_flag(command, "--allow-failed") and not allow_failed_receipts:
        raise EvidenceError(f"{label} allowed failed receipts")
    if _command_has_flag(command, "--allow-legacy-colr007") and not allow_legacy_colr007:
        raise EvidenceError(f"{label} used --allow-legacy-colr007")


def _check_stage_script(stage_name: str, command: list[str], label: str) -> None:
    expected = EXPECTED_STAGE_SCRIPTS.get(stage_name)
    if expected is None:
        raise EvidenceError(f"{label}.name has unsupported canary stage {stage_name!r}")
    if not _command_has_script(command, expected):
        raise EvidenceError(f"{label}.command does not invoke {expected}")


def _check_canary_stage_sequence(stage_names: list[str], label: str) -> None:
    stage_name_set = set(stage_names)
    unsupported = sorted(stage_name_set - REQUIRED_CANARY_STAGES)
    if unsupported:
        raise EvidenceError(
            f"{label} contains unsupported canary stages: " + ", ".join(unsupported)
        )
    expected = [
        stage_name
        for stage_name in EXPECTED_CANARY_STAGE_ORDER
        if stage_name in stage_name_set
    ]
    if stage_names != expected:
        raise EvidenceError(
            f"{label} stages must follow canary order: "
            + ", ".join(EXPECTED_CANARY_STAGE_ORDER)
        )


def _check_stage_command_flags(stage_name: str, command: list[str], label: str) -> None:
    allowed = EXPECTED_STAGE_FLAGS.get(stage_name)
    if allowed is None:
        raise EvidenceError(f"{label}.name has unsupported canary stage {stage_name!r}")
    local_only = LOCAL_DIAGNOSTIC_STAGE_FLAGS.get(stage_name, set())
    boolean_flags = STAGE_BOOLEAN_FLAGS.get(stage_name, set())
    value_offsets = _command_separate_value_offsets(
        command,
        allowed - boolean_flags,
        label,
    )
    for offset, item in enumerate(command):
        if offset in value_offsets:
            continue
        if not item.startswith("--"):
            continue
        flag = item.split("=", 1)[0]
        if flag in local_only:
            raise EvidenceError(
                f"{label}.command[{offset}] uses local diagnostic flag {flag!r}; "
                "production evidence must include persisted source records"
            )
        if flag not in allowed:
            raise EvidenceError(f"{label}.command[{offset}] uses unsupported flag {flag!r}")
        if item.startswith(flag + "=") and flag in boolean_flags:
            raise EvidenceError(
                f"{label}.command[{offset}] boolean flag {flag} must not use =value"
            )
        if (
            flag in boolean_flags
            and item == flag
            and offset + 1 < len(command)
            and offset + 1 not in value_offsets
            and not command[offset + 1].startswith("--")
        ):
            raise EvidenceError(
                f"{label}.command[{offset}] boolean flag {flag} must not use a value"
            )
    for flag in sorted(STAGE_SINGLETON_FLAGS.get(stage_name, set())):
        if _command_flag_count(command, flag) > 1:
            raise EvidenceError(f"{label}.command must contain at most one {flag}")
    _check_numeric_command_flags(stage_name, command, label)
    _check_path_command_flags(stage_name, command, label)
    _check_required_command_flags(stage_name, command, label)


def _check_receipt_dir_binding(command: list[str], receipt_dir: str, label: str) -> None:
    recorded = _validate_artifact_path(receipt_dir, f"{label}.receipt_dir")
    values = _command_flag_values(command, "--receipt-dir", label)
    if len(values) != 1:
        raise EvidenceError(f"{label}.command must contain exactly one --receipt-dir")
    value_offset, command_value = values[0]
    command_receipt_dir = _validate_artifact_path(
        command_value,
        f"{label}.command[{value_offset}]",
    )
    if command_receipt_dir != recorded:
        raise EvidenceError(f"{label}.receipt_dir does not match command --receipt-dir")


def _verify_receipt_stdout(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    if _required_bool(stage, "stdout_truncated", label):
        raise EvidenceError(f"{label}.stdout_preview is truncated")
    stdout = stage.get("stdout_preview")
    if not isinstance(stdout, str) or not stdout.strip():
        raise EvidenceError(f"{label}.stdout_preview must contain receipt verifier JSON")
    try:
        receipt_summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{label}.stdout_preview is not valid receipt verifier JSON") from error
    _reject_json_surrogates(receipt_summary)
    receipt_obj = _require_object(receipt_summary, f"{label}.stdout_preview")
    return _verify_receipt_verifier_summary(receipt_obj, f"{label}.stdout_preview", args)


def _check_stage_output_not_truncated(stage: dict[str, Any], label: str) -> None:
    if _required_bool(stage, "stdout_truncated", label):
        raise EvidenceError(f"{label}.stdout_preview is truncated")
    if _required_bool(stage, "stderr_truncated", label):
        raise EvidenceError(f"{label}.stderr_preview is truncated")


def _stage_summary(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
    *,
    canary_started_at: dt.datetime,
    canary_finished_at: dt.datetime,
) -> dict[str, Any]:
    _reject_unknown_keys(stage, CANARY_STAGE_KEYS, label)
    name = _required_stage_name(stage, "name", label)
    started_at_raw, started_at = _required_timestamp(stage, "started_at", label)
    finished_at_raw, finished_at = _required_timestamp(stage, "finished_at", label)
    if finished_at < started_at:
        raise EvidenceError(f"{label}.finished_at must not be before started_at")
    if started_at < canary_started_at or finished_at > canary_finished_at:
        raise EvidenceError(f"{label} timestamp window must be inside canary window")
    skipped = _required_bool(stage, "skipped", label)
    if skipped:
        raise EvidenceError(f"{label} was skipped")
    if _required_bool(stage, "timed_out", label):
        raise EvidenceError(f"{label} timed out")
    returncode = stage.get("returncode")
    if isinstance(returncode, bool) or not isinstance(returncode, int):
        raise EvidenceError(f"{label}.returncode must be an integer")
    if returncode != 0:
        raise EvidenceError(f"{label} failed with returncode {returncode}")
    _check_stage_output_not_truncated(stage, label)
    command = _require_list(stage.get("command"), f"{label}.command")
    if not all(isinstance(item, str) for item in command):
        raise EvidenceError(f"{label}.command must contain strings")
    _check_command_policy(
        command,
        label,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    _check_stage_script(name, command, label)
    if name in {"rail", "notary"}:
        receipt_dir = stage.get("receipt_dir")
        if not isinstance(receipt_dir, str) or not receipt_dir.strip():
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        _check_receipt_dir_binding(command, receipt_dir, label)
    _check_stage_command_flags(name, command, label)
    if name == "verify":
        receipt_dirs = [
            _validate_artifact_path(value, f"{label}.command[{offset}]")
            for offset, value in _command_flag_values(command, "--receipt-dir", label)
        ]
        result = {
            "name": name,
            "_started_at": started_at,
            "_finished_at": finished_at,
            "_receipt_dirs": receipt_dirs,
            "started_at": started_at_raw,
            "finished_at": finished_at_raw,
        }
        if (
            not _command_has_flag(command, "--require-source-files")
            and not args.allow_receipt_source_missing
        ):
            raise EvidenceError(f"{label} did not require receipt source files")
        result["receipt_summary"] = _verify_receipt_stdout(stage, label, args)
        return result
    return {
        "name": name,
        "_started_at": started_at,
        "_finished_at": finished_at,
        "_dry_run": _command_has_flag(command, "--dry-run"),
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
        "receipt_dir": _validate_artifact_path(receipt_dir, f"{label}.receipt_dir")
        if name in {"rail", "notary"}
        else None,
    }


def _planned_stage_summary(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> str:
    _reject_unknown_keys(stage, CANARY_PLANNED_STAGE_KEYS, label)
    name = _required_stage_name(stage, "name", label)
    dry_run = _required_bool(stage, "dry_run", label)
    if dry_run and not args.allow_dry_run:
        raise EvidenceError(f"{label} planned a dry-run stage")
    command = _require_list(stage.get("command"), f"{label}.command")
    if not all(isinstance(item, str) for item in command):
        raise EvidenceError(f"{label}.command must contain strings")
    _check_command_policy(
        command,
        label,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    _check_stage_script(name, command, label)
    if name in {"rail", "notary"}:
        receipt_dir = stage.get("receipt_dir")
        if not isinstance(receipt_dir, str) or not receipt_dir.strip():
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        _check_receipt_dir_binding(command, receipt_dir, label)
    _check_stage_command_flags(name, command, label)
    if (
        name == "verify"
        and not _command_has_flag(command, "--require-source-files")
        and not args.allow_receipt_source_missing
    ):
        raise EvidenceError(f"{label} did not require receipt source files")
    return name


def verify_canary_summary(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify one archived canary summary and return compact evidence metadata."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _reject_unknown_keys(summary, CANARY_SUMMARY_KEYS, str(path))
    _check_no_secret_material(summary)
    version = summary.get("version")
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version != CANARY_SUMMARY_VERSION
    ):
        raise EvidenceError(f"{path}.version must be {CANARY_SUMMARY_VERSION}")

    provider = _required_string(summary, "provider", str(path))
    environment = _required_string(summary, "environment", str(path))
    _reject_secret_looking_identifier(provider, f"{path}.provider")
    _reject_secret_looking_identifier(environment, f"{path}.environment")
    config_path = _validate_config_path(
        _required_string(summary, "config_path", str(path)),
        f"{path}.config_path",
    )
    started_at_raw, started_at = _required_timestamp(summary, "started_at", str(path))
    finished_at_raw, finished_at = _required_timestamp(summary, "finished_at", str(path))
    if finished_at < started_at:
        raise EvidenceError(f"{path}.finished_at must not be before started_at")
    _reject_stale_timestamp(
        finished_at,
        max_age_days=args.max_canary_age_days,
        label=f"{path}.finished_at",
    )
    if args.provider is not None and provider != args.provider:
        raise EvidenceError(f"{path}.provider is {provider!r}, expected {args.provider!r}")
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(
            f"{path}.environment is {environment!r}, expected {args.environment!r}"
        )

    ok = _required_bool(summary, "ok", str(path))
    if not ok:
        raise EvidenceError(f"{path} is not an ok canary summary")
    plan_only = _required_bool(summary, "plan_only", str(path))
    if plan_only and not args.allow_plan_only:
        raise EvidenceError(f"{path} is plan-only evidence")
    policy = _require_object(summary.get("policy"), f"{path}.policy")
    _reject_unknown_keys(policy, CANARY_POLICY_KEYS, f"{path}.policy")
    require_explicit_policy = _required_bool(
        policy,
        "require_explicit_policy",
        f"{path}.policy",
    )
    if not require_explicit_policy:
        raise EvidenceError(f"{path} was not produced with --require-explicit-policy")

    stage_results: list[dict[str, Any]] = []
    if plan_only:
        if "stages" in summary:
            raise EvidenceError(f"{path}.stages must be omitted for plan-only evidence")
        stages = _require_list(summary.get("planned_stages"), f"{path}.planned_stages")
        stage_names = [
            _planned_stage_summary(
                _require_object(stage, f"{path}.planned_stages[{offset}]"),
                f"{path}.planned_stages[{offset}]",
                args,
            )
            for offset, stage in enumerate(stages)
        ]
    else:
        if "planned_stages" in summary:
            raise EvidenceError(f"{path}.planned_stages must be omitted for executed evidence")
        stages = _require_list(summary.get("stages"), f"{path}.stages")
        stage_results = [
            _stage_summary(
                _require_object(stage, f"{path}.stages[{offset}]"),
                f"{path}.stages[{offset}]",
                args,
                canary_started_at=started_at,
                canary_finished_at=finished_at,
            )
            for offset, stage in enumerate(stages)
        ]
        stage_names = [stage["name"] for stage in stage_results]

    if len(stage_names) != len(set(stage_names)):
        raise EvidenceError(f"{path} contains duplicate canary stages")
    _check_canary_stage_sequence(stage_names, str(path))
    previous_finished_at: dt.datetime | None = None
    for offset, stage in enumerate(stage_results):
        if previous_finished_at is not None and stage["_started_at"] < previous_finished_at:
            raise EvidenceError(
                f"{path}.stages[{offset}].started_at must not be before previous stage finished_at"
            )
        previous_finished_at = stage["_finished_at"]
    stage_name_set = set(stage_names)
    if args.allow_partial_canary:
        if "verify" not in stage_name_set:
            raise EvidenceError(f"{path} is missing verify stage")
        if not ({"rail", "notary"} & stage_name_set):
            raise EvidenceError(f"{path} must include rail or notary stage")
    else:
        missing = sorted(REQUIRED_CANARY_STAGES - stage_name_set)
        if missing:
            raise EvidenceError(
                f"{path} is missing required canary stages: {', '.join(missing)}"
            )
    verify_receipt_dirs = next(
        (
            stage["_receipt_dirs"]
            for stage in stage_results
            if stage["name"] == "verify"
        ),
        [],
    )
    for stage in stage_results:
        if stage["name"] not in {"rail", "notary"} or stage.get("_dry_run"):
            continue
        receipt_dir = stage.get("receipt_dir")
        if receipt_dir is not None and receipt_dir not in verify_receipt_dirs:
            raise EvidenceError(
                f"{path}.stages verify command does not include {stage['name']} receipt_dir"
            )
    receipt_summary = next(
        (
            stage["receipt_summary"]
            for stage in stage_results
            if stage["name"] == "verify" and "receipt_summary" in stage
        ),
        None,
    )

    return {
        "version": version,
        "path": str(path),
        "config_path": config_path,
        "provider": provider,
        "environment": environment,
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
        "plan_only": plan_only,
        "require_explicit_policy": require_explicit_policy,
        "stage_names": stage_names,
        "stage_windows": [
            {
                "name": stage["name"],
                "started_at": stage["started_at"],
                "finished_at": stage["finished_at"],
            }
            for stage in stage_results
        ],
        "receipt_summary": receipt_summary,
        "summary_sha256": digest,
    }


def _check_clean_http_url(
    url: str,
    label: str,
    *,
    allow_insecure_http: bool,
    reject_local_hosts: bool = False,
) -> None:
    if len(url) > MAX_HTTP_URL_CHARS:
        raise EvidenceError(f"{label} must be no longer than {MAX_HTTP_URL_CHARS} characters")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise EvidenceError(f"{label} must not contain control characters")
    _reject_url_percent_encoding_smuggling(url, label)
    if any(ch.isspace() for ch in url):
        raise EvidenceError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise EvidenceError(f"{label} is not a valid URL") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise EvidenceError(f"{label} uses insecure HTTP URL")
        raise EvidenceError(f"{label} must use HTTPS URL")
    try:
        port = parsed.port
    except ValueError as error:
        raise EvidenceError(f"{label} has invalid port") from error
    port_text = _raw_url_port_text(parsed)
    if port_text == "":
        raise EvidenceError(f"{label} must not include an empty port")
    if port_text is not None:
        if len(port_text) > 1 and port_text.startswith("0"):
            raise EvidenceError(f"{label} port must not contain leading zeros")
        if port == 0:
            raise EvidenceError(f"{label} port must be positive")
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise EvidenceError(f"{label} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise EvidenceError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise EvidenceError(f"{label} must not contain credentials")
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise EvidenceError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise EvidenceError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise EvidenceError(f"{label} host must not end with a dot")
    _reject_secret_looking_identifier(raw_host, f"{label} host")
    _validate_host_labels(raw_host, label)
    if parsed.params or parsed.query or parsed.fragment:
        raise EvidenceError(f"{label} must not contain params, query, or fragment")
    _validate_url_path(parsed, label)
    hostname = hostname.strip().lower()
    if reject_local_hosts and _host_uses_reserved_placeholder_suffix(hostname):
        raise EvidenceError(f"{label} must not use reserved placeholder hostnames")
    if reject_local_hosts and not allow_insecure_http:
        if hostname == "localhost" or hostname.endswith(".localhost"):
            raise EvidenceError(f"{label} must not use localhost")
        if _host_uses_rebinding_suffix(hostname):
            raise EvidenceError(f"{label} must not use local/private rebinding hostnames")
        try:
            address = ipaddress.ip_address(hostname)
        except ValueError:
            return
        if not address.is_global:
            raise EvidenceError(f"{label} must not use local, private, or reserved IP addresses")
        if _address_embeds_non_global_ipv4(address):
            raise EvidenceError(f"{label} must not embed local, private, or reserved IPv4 addresses")


def _host_uses_rebinding_suffix(hostname: str) -> bool:
    return hostname in LOCAL_REBINDING_HOST_SUFFIXES or any(
        hostname.endswith("." + suffix) for suffix in LOCAL_REBINDING_HOST_SUFFIXES
    )


def _host_uses_reserved_placeholder_suffix(hostname: str) -> bool:
    reserved_hosts = PLACEHOLDER_TRUST_SOURCE_HOSTS | TEMPLATE_CANARY_ENDPOINT_HOSTS
    return hostname in reserved_hosts or any(
        hostname.endswith("." + suffix) for suffix in reserved_hosts
    )


def _address_embeds_non_global_ipv4(address: ipaddress.IPv4Address | ipaddress.IPv6Address) -> bool:
    embedded: ipaddress.IPv4Address | None = None
    if isinstance(address, ipaddress.IPv6Address):
        if address.ipv4_mapped is not None:
            embedded = address.ipv4_mapped
        elif address in NAT64_WELL_KNOWN_PREFIX or address in IPV4_COMPATIBLE_IPV6_PREFIX:
            embedded = ipaddress.IPv4Address(int(address) & 0xFFFF_FFFF)
        elif address.sixtofour is not None:
            embedded = address.sixtofour
        elif address.teredo is not None:
            embedded = address.teredo[1]
    return embedded is not None and not embedded.is_global


def _raw_url_host(parsed: urllib.parse.ParseResult) -> str:
    netloc = parsed.netloc.rsplit("@", 1)[-1]
    if netloc.startswith("["):
        bracket = netloc.find("]")
        if bracket != -1:
            return netloc[1:bracket]
    return netloc.rsplit(":", 1)[0]


def _raw_url_port_text(parsed: urllib.parse.ParseResult) -> str | None:
    netloc = parsed.netloc.rsplit("@", 1)[-1]
    if netloc.startswith("["):
        bracket = netloc.find("]")
        if bracket == -1:
            return None
        remainder = netloc[bracket + 1 :]
        if remainder.startswith(":"):
            return remainder[1:]
        return None
    if ":" in netloc:
        return netloc.rsplit(":", 1)[1]
    return None


def _reject_url_percent_encoding_smuggling(url: str, label: str) -> None:
    index = 0
    while True:
        index = url.find("%", index)
        if index == -1:
            return
        token = url[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise EvidenceError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise EvidenceError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        index += 3


def _validate_host_labels(raw_host: str, label: str) -> None:
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise EvidenceError(f"{label} host must be a valid IP address")
    if len(raw_host) > 253:
        raise EvidenceError(f"{label} host must be at most 253 characters")
    _reject_legacy_ipv4_host_notation(raw_host, label)
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise EvidenceError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise EvidenceError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise EvidenceError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise EvidenceError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise EvidenceError(
                f"{label} host labels must use lowercase ASCII letters, digits, or hyphens"
            )


def _reject_legacy_ipv4_host_notation(raw_host: str, label: str) -> None:
    parts = raw_host.split(".")
    if len(parts) > 4:
        return
    saw_hex_part = False
    for part in parts:
        if part.startswith("0x"):
            digits = part[2:]
            if not digits or any(ch not in "0123456789abcdef" for ch in digits):
                return
            saw_hex_part = True
        elif not part.isdigit():
            return
    if saw_hex_part:
        raise EvidenceError(f"{label} host must not use legacy IPv4 numeric notation")


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if "\\" in path:
        raise EvidenceError(f"{label} path must use forward slashes")
    if ";" in path:
        raise EvidenceError(f"{label} path must not contain semicolon parameters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise EvidenceError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise EvidenceError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
        raise EvidenceError(f"{label} path must not contain secret-looking material")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise EvidenceError(f"{label} path must not contain encoded dot or separator characters")
    if "%3b" in lowered:
        raise EvidenceError(f"{label} path must not contain encoded semicolon parameters")
    if any(token in lowered for token in ("%23", "%3a", "%3f", "%40", "%5b", "%5d")):
        raise EvidenceError(f"{label} path must not contain encoded URL delimiter characters")
    if "%25" in lowered:
        raise EvidenceError(f"{label} path must not contain encoded percent characters")


def _check_https_url(url: str, label: str, *, allow_insecure_http: bool) -> None:
    _check_clean_http_url(
        url,
        label,
        allow_insecure_http=allow_insecure_http,
        reject_local_hosts=True,
    )


def _trust_source_text_is_placeholder(value: str) -> bool:
    lowered = value.lower()
    return any(marker in lowered for marker in PLACEHOLDER_TRUST_SOURCE_MARKERS)


def _reject_placeholder_trust_source_text(value: str, label: str) -> None:
    if _trust_source_text_is_placeholder(value):
        raise EvidenceError(f"{label} must not contain placeholder production metadata")


def _trust_source_url_uses_placeholder_host(url: str) -> bool:
    parsed = urllib.parse.urlparse(url)
    hostname = (parsed.hostname or "").lower()
    return hostname in PLACEHOLDER_TRUST_SOURCE_HOSTS or any(
        hostname.endswith("." + host) for host in PLACEHOLDER_TRUST_SOURCE_HOSTS
    )


def _reject_placeholder_trust_source_url(url: str, label: str) -> None:
    if _trust_source_url_uses_placeholder_host(url):
        raise EvidenceError(f"{label} must not use reserved placeholder provenance")


def _parse_timestamp(value: Any, label: str) -> dt.datetime:
    if not isinstance(value, str):
        raise EvidenceError(f"{label} must be recorded")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise EvidenceError(f"{label} must not contain control characters")
    if not value.strip():
        raise EvidenceError(f"{label} must be recorded")
    if value != value.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    text = value
    normalized = text[:-1] + "+00:00" if text.endswith("Z") else text
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise EvidenceError(f"{label} must be an ISO 8601 timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise EvidenceError(f"{label} must include a timezone offset")
    parsed_utc = parsed.astimezone(dt.UTC)
    if parsed_utc > dt.datetime.now(dt.UTC):
        raise EvidenceError(f"{label} must not be in the future")
    return parsed_utc


def _check_retrieved_at(value: str, label: str, args: argparse.Namespace) -> None:
    retrieved_at = _parse_timestamp(value, label)
    _reject_stale_timestamp(
        retrieved_at,
        max_age_days=args.max_trust_source_age_days,
        label=label,
    )


def _trust_source_is_stale_for_budget(
    source: dict[str, str],
    *,
    max_age_days: int,
    label: str,
) -> bool:
    retrieved_at = _parse_timestamp(source["retrieved_at"], f"{label}.retrieved_at")
    cutoff = dt.datetime.now(dt.UTC) - dt.timedelta(days=max_age_days)
    return retrieved_at < cutoff


def _computed_profile_json_emittable(
    *,
    allow_synthetic_der: bool,
    allow_record_only: bool,
    allow_insecure_source_url: bool,
    max_source_age_days: int | None,
    bundle_summaries: list[dict[str, Any]],
) -> bool:
    if allow_synthetic_der or allow_record_only or allow_insecure_source_url:
        return False
    if max_source_age_days is None:
        return False
    if not bundle_summaries:
        return False
    for offset, bundle in enumerate(bundle_summaries):
        source = bundle["source"]
        if source is None:
            return False
        if (
            _trust_source_text_is_placeholder(source["authority"])
            or _trust_source_text_is_placeholder(source["version"])
            or _trust_source_url_uses_placeholder_host(source["url"])
        ):
            return False
        if _trust_source_is_stale_for_budget(
            source,
            max_age_days=max_source_age_days,
            label=f"bundles[{offset}].source",
        ):
            return False
    return True


def _check_trust_bundle(
    bundle: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    _reject_unknown_keys(bundle, TRUST_BUNDLE_KEYS, label)
    profile_id = _required_profile_id(bundle, "profile_id", label)
    rail = _required_rail(bundle, "rail", label)
    environment = _required_string(bundle, "environment", label)
    _reject_secret_looking_identifier(environment, f"{label}.environment")
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(
            f"{label}.environment is {environment!r}, expected {args.environment!r}"
        )
    policy = _required_string(bundle, "embedded_signature_policy", label)
    _reject_secret_looking_identifier(policy, f"{label}.embedded_signature_policy")
    if policy not in TRUST_SIGNATURE_POLICIES:
        raise EvidenceError(f"{label}.embedded_signature_policy is unsupported")
    if policy != REQUIRE_VERIFIED and not args.allow_record_only_trust:
        raise EvidenceError(f"{label}.embedded_signature_policy is {policy!r}")

    source_summary: dict[str, str] | None = None
    bundle_sha256 = bundle.get("bundle_sha256")
    if isinstance(bundle_sha256, str):
        _reject_secret_looking_identifier(bundle_sha256, f"{label}.bundle_sha256")
    if not _is_lower_sha256(bundle_sha256):
        raise EvidenceError(f"{label}.bundle_sha256 must be a canonical SHA-256")
    if "source" not in bundle:
        raise EvidenceError(f"{label}.source must be explicitly recorded")
    source = bundle["source"]
    if source is None:
        if not args.allow_missing_trust_source:
            raise EvidenceError(f"{label}.source is required for production evidence")
    else:
        source_obj = _require_object(source, f"{label}.source")
        _reject_unknown_keys(source_obj, TRUST_SOURCE_KEYS, f"{label}.source")
        authority = _required_string(source_obj, "authority", f"{label}.source")
        version = _required_string(source_obj, "version", f"{label}.source")
        _reject_secret_looking_identifier(authority, f"{label}.source.authority")
        _reject_secret_looking_identifier(version, f"{label}.source.version")
        url = source_obj.get("url")
        if not isinstance(url, str) or not url.strip():
            raise EvidenceError(f"{label}.source.url must be recorded")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
            raise EvidenceError(f"{label}.source.url must not contain control characters")
        if url != url.strip():
            raise EvidenceError(f"{label}.source.url must not have surrounding whitespace")
        _check_https_url(
            url,
            f"{label}.source.url",
            allow_insecure_http=args.allow_insecure_http,
        )
        _reject_placeholder_trust_source_text(authority, f"{label}.source.authority")
        _reject_placeholder_trust_source_text(version, f"{label}.source.version")
        _reject_placeholder_trust_source_url(url, f"{label}.source.url")
        retrieved_at = source_obj.get("retrieved_at")
        if not isinstance(retrieved_at, str):
            raise EvidenceError(f"{label}.source.retrieved_at must be recorded")
        if not retrieved_at.strip():
            raise EvidenceError(f"{label}.source.retrieved_at must be recorded")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in retrieved_at):
            raise EvidenceError(
                f"{label}.source.retrieved_at must not contain control characters"
            )
        if retrieved_at != retrieved_at.strip():
            raise EvidenceError(
                f"{label}.source.retrieved_at must not have surrounding whitespace"
            )
        _check_retrieved_at(
            retrieved_at,
            f"{label}.source.retrieved_at",
            args,
        )
        source_summary = {
            "authority": authority,
            "version": version,
            "url": url,
            "retrieved_at": retrieved_at,
        }

    material = _require_object(bundle.get("material"), f"{label}.material")
    _reject_unknown_keys(material, TRUST_MATERIAL_KEYS, f"{label}.material")
    signature_pin_count = _required_nonnegative_int(
        material,
        "signature_public_key_pin_count",
        f"{label}.material",
    )
    x509_anchor_pin_count = _required_nonnegative_int(
        material,
        "x509_trust_anchor_pin_count",
        f"{label}.material",
    )
    revoked_pin_count = _required_nonnegative_int(
        material,
        "revoked_certificate_pin_count",
        f"{label}.material",
    )
    if signature_pin_count + x509_anchor_pin_count == 0:
        raise EvidenceError(f"{label} has no signature public-key or X.509 trust pins")
    trust_anchor_der_entries = _required_der_summary_entries(
        bundle,
        "x509_trust_anchors",
        label,
    )
    if len(trust_anchor_der_entries) > x509_anchor_pin_count:
        raise EvidenceError(
            f"{label}.x509_trust_anchors length exceeds material X.509 trust-anchor pin count"
        )
    revoked_der_entries = _required_der_summary_entries(
        bundle,
        "revoked_certificates",
        label,
    )
    if len(revoked_der_entries) > revoked_pin_count:
        raise EvidenceError(
            f"{label}.revoked_certificates length exceeds material revoked-certificate pin count"
        )

    profile_overrides = _require_object(
        bundle.get("profile_overrides"),
        f"{label}.profile_overrides",
    )
    _reject_unknown_keys(
        profile_overrides,
        TRUST_PROFILE_OVERRIDE_KEYS,
        f"{label}.profile_overrides",
    )
    override_id = _required_string(
        profile_overrides,
        "id",
        f"{label}.profile_overrides",
    )
    if override_id != profile_id:
        raise EvidenceError(f"{label}.profile_overrides.id does not match profile_id")
    override_rail = _required_string(
        profile_overrides,
        "rail",
        f"{label}.profile_overrides",
    )
    if override_rail != rail:
        raise EvidenceError(f"{label}.profile_overrides.rail does not match rail")
    override_policy = _required_string(
        profile_overrides,
        "embedded_signature_policy",
        f"{label}.profile_overrides",
    )
    _reject_secret_looking_identifier(
        override_policy,
        f"{label}.profile_overrides.embedded_signature_policy",
    )
    if override_policy != policy:
        raise EvidenceError(
            f"{label}.profile_overrides.embedded_signature_policy does not match embedded_signature_policy"
        )
    override_public_pins = _required_sha256_list(
        profile_overrides,
        "signature_public_key_sha256_pins",
        f"{label}.profile_overrides",
    )
    override_legacy_public_pins = _required_sha256_list(
        profile_overrides,
        "trusted_public_key_sha256",
        f"{label}.profile_overrides",
    )
    _reject_sha256_overlap(
        override_public_pins,
        override_legacy_public_pins,
        f"{label}.profile_overrides.signature_public_key_sha256_pins/trusted_public_key_sha256",
    )
    if len(override_public_pins) + len(override_legacy_public_pins) != signature_pin_count:
        raise EvidenceError(
            f"{label}.profile_overrides public-key pin count does not match material"
        )
    override_anchor_pins = _required_sha256_list(
        profile_overrides,
        "x509_trust_anchor_sha256_pins",
        f"{label}.profile_overrides",
    )
    override_legacy_anchor_pins = _required_sha256_list(
        profile_overrides,
        "trusted_certificate_sha256",
        f"{label}.profile_overrides",
    )
    _reject_sha256_overlap(
        override_anchor_pins,
        override_legacy_anchor_pins,
        f"{label}.profile_overrides.x509_trust_anchor_sha256_pins/trusted_certificate_sha256",
    )
    if len(override_anchor_pins) + len(override_legacy_anchor_pins) != x509_anchor_pin_count:
        raise EvidenceError(
            f"{label}.profile_overrides X.509 trust-anchor pin count does not match material"
        )
    override_revoked_pins = _required_sha256_list(
        profile_overrides,
        "revoked_certificate_sha256",
        f"{label}.profile_overrides",
    )
    if len(override_revoked_pins) != revoked_pin_count:
        raise EvidenceError(
            f"{label}.profile_overrides revoked-certificate pin count does not match material"
        )
    _reject_sha256_overlap(
        override_anchor_pins + override_legacy_anchor_pins,
        override_revoked_pins,
        f"{label}.profile_overrides trusted/revoked certificate pins",
    )
    _require_summary_digests_in_pins(
        trust_anchor_der_entries,
        override_anchor_pins + override_legacy_anchor_pins,
        f"{label}.x509_trust_anchors",
        f"{label}.profile_overrides X.509 trust-anchor pins",
    )
    _require_summary_digests_in_pins(
        revoked_der_entries,
        override_revoked_pins,
        f"{label}.revoked_certificates",
        f"{label}.profile_overrides.revoked_certificate_sha256",
    )
    policy_oids = _required_oid_list(
        profile_overrides,
        "x509_required_certificate_policy_oids",
        f"{label}.profile_overrides",
    )
    policy_oid_count = _required_nonnegative_int(
        material,
        "x509_required_certificate_policy_oid_count",
        f"{label}.material",
    )
    if len(policy_oids) != policy_oid_count:
        raise EvidenceError(
            f"{label}.profile_overrides certificate-policy OID count does not match material"
        )
    crl_required = _required_bool(
        profile_overrides,
        "x509_require_crl_revocation_check",
        f"{label}.profile_overrides",
    )
    ocsp_required = _required_bool(
        profile_overrides,
        "x509_require_ocsp_revocation_check",
        f"{label}.profile_overrides",
    )
    x509_crl_count = _required_nonnegative_int(
        material,
        "x509_crl_count",
        f"{label}.material",
    )
    x509_ocsp_response_count = _required_nonnegative_int(
        material,
        "x509_ocsp_response_count",
        f"{label}.material",
    )
    if crl_required and x509_crl_count == 0:
        raise EvidenceError(f"{label} requires CRL revocation checking but has no CRLs")
    if ocsp_required and x509_ocsp_response_count == 0:
        raise EvidenceError(f"{label} requires OCSP revocation checking but has no OCSP responses")
    crl_der_entries = _required_der_summary_entries(
        bundle,
        "x509_crls",
        label,
    )
    if len(crl_der_entries) != x509_crl_count:
        raise EvidenceError(f"{label}.x509_crls length does not match material")
    ocsp_der_entries = _required_der_summary_entries(
        bundle,
        "x509_ocsp_responses",
        label,
    )
    if len(ocsp_der_entries) != x509_ocsp_response_count:
        raise EvidenceError(f"{label}.x509_ocsp_responses length does not match material")
    crl_der = _required_canonical_base64_list(
        profile_overrides,
        "x509_crl_der_base64",
        f"{label}.profile_overrides",
    )
    if len(crl_der) != x509_crl_count:
        raise EvidenceError(f"{label}.profile_overrides CRL DER count does not match material")
    _require_override_der_matches_summary(
        crl_der,
        crl_der_entries,
        f"{label}.profile_overrides.x509_crl_der_base64",
        f"{label}.x509_crls",
    )
    ocsp_der = _required_canonical_base64_list(
        profile_overrides,
        "x509_ocsp_response_der_base64",
        f"{label}.profile_overrides",
    )
    if len(ocsp_der) != x509_ocsp_response_count:
        raise EvidenceError(f"{label}.profile_overrides OCSP DER count does not match material")
    _require_override_der_matches_summary(
        ocsp_der,
        ocsp_der_entries,
        f"{label}.profile_overrides.x509_ocsp_response_der_base64",
        f"{label}.x509_ocsp_responses",
    )

    return {
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "bundle_sha256": bundle_sha256,
        "source": source_summary,
        "embedded_signature_policy": policy,
        "signature_public_key_pin_count": signature_pin_count,
        "x509_trust_anchor_pin_count": x509_anchor_pin_count,
        "x509_trust_anchor_der": _compact_der_entries(trust_anchor_der_entries),
        "revoked_certificate_pin_count": revoked_pin_count,
        "revoked_certificate_der": _compact_der_entries(revoked_der_entries),
        "x509_required_certificate_policy_oid_count": policy_oid_count,
        "x509_require_crl_revocation_check": crl_required,
        "x509_crl_count": x509_crl_count,
        "x509_crl_der": _compact_der_entries(crl_der_entries),
        "x509_require_ocsp_revocation_check": ocsp_required,
        "x509_ocsp_response_count": x509_ocsp_response_count,
        "x509_ocsp_response_der": _compact_der_entries(ocsp_der_entries),
    }


def verify_trust_summary(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify one archived trust-bundle summary and return compact metadata."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _reject_unknown_keys(summary, TRUST_SUMMARY_KEYS, str(path))
    _check_no_secret_material(summary)
    version = summary.get("version")
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version != TRUST_SUMMARY_VERSION
    ):
        raise EvidenceError(f"{path}.version must be {TRUST_SUMMARY_VERSION}")
    verified_at_raw, verified_at = _required_timestamp(summary, "verified_at", str(path))
    _reject_stale_timestamp(
        verified_at,
        max_age_days=args.max_trust_age_days,
        label=f"{path}.verified_at",
    )

    allow_synthetic_der = _required_bool(summary, "allow_synthetic_der", str(path))
    allow_record_only = _required_bool(summary, "allow_record_only", str(path))
    allow_insecure_source_url = _required_bool(summary, "allow_insecure_source_url", str(path))
    profile_json_emitted = _required_bool(summary, "profile_json_emitted", str(path))
    profile_json_emittable = _required_bool(summary, "profile_json_emittable", str(path))
    if "max_source_age_days" not in summary:
        raise EvidenceError(f"{path}.max_source_age_days must be recorded")
    if summary["max_source_age_days"] is None:
        max_source_age_days = None
    else:
        max_source_age_days = _required_positive_int_field(
            summary,
            "max_source_age_days",
            str(path),
        )
    if profile_json_emitted:
        profile_json_sha256 = _required_sha256(summary, "profile_json_sha256", str(path))
    else:
        if "profile_json_sha256" not in summary:
            raise EvidenceError(f"{path}.profile_json_sha256 must be null when profile JSON was not emitted")
        if summary["profile_json_sha256"] is not None:
            raise EvidenceError(f"{path}.profile_json_sha256 must be null when profile JSON was not emitted")
        profile_json_sha256 = None
    if allow_synthetic_der and not args.allow_synthetic_trust:
        raise EvidenceError(f"{path} was verified with --allow-synthetic-der")
    if allow_record_only and not args.allow_record_only_trust:
        raise EvidenceError(f"{path} was verified with --allow-record-only")
    if allow_insecure_source_url and not args.allow_insecure_http:
        raise EvidenceError(f"{path} was verified with --allow-insecure-source-url")
    if profile_json_emittable:
        if max_source_age_days is None:
            raise EvidenceError(f"{path}.max_source_age_days must be a positive integer")
        if max_source_age_days > args.max_trust_source_age_days:
            raise EvidenceError(
                f"{path}.max_source_age_days is weaker than "
                "--max-trust-source-age-days"
            )
    if not profile_json_emitted and not args.allow_profile_json_not_emitted:
        raise EvidenceError(f"{path} did not emit profile JSON")

    verified_bundles = summary.get("verified_bundles")
    if isinstance(verified_bundles, bool) or not isinstance(verified_bundles, int) or verified_bundles <= 0:
        raise EvidenceError(f"{path}.verified_bundles must be a positive integer")
    bundles = _require_list(summary.get("bundles"), f"{path}.bundles")
    if len(bundles) != verified_bundles:
        raise EvidenceError(f"{path}.bundles length does not match verified_bundles")
    bundle_objects = [
        _require_object(bundle, f"{path}.bundles[{offset}]")
        for offset, bundle in enumerate(bundles)
    ]
    bundle_summaries = [
        _check_trust_bundle(
            bundle,
            f"{path}.bundles[{offset}]",
            args,
        )
        for offset, bundle in enumerate(bundle_objects)
    ]
    profile_json_non_emittable_allowed = (
        (allow_synthetic_der and args.allow_synthetic_trust)
        or (allow_record_only and args.allow_record_only_trust)
        or (allow_insecure_source_url and args.allow_insecure_http)
        or (
            args.allow_missing_trust_source
            and any(bundle.get("source") is None for bundle in bundle_summaries)
        )
    )
    if not profile_json_emittable and not profile_json_non_emittable_allowed:
        raise EvidenceError(f"{path} cannot emit production profile JSON")
    computed_profile_json_emittable = _computed_profile_json_emittable(
        allow_synthetic_der=allow_synthetic_der,
        allow_record_only=allow_record_only,
        allow_insecure_source_url=allow_insecure_source_url,
        max_source_age_days=max_source_age_days,
        bundle_summaries=bundle_summaries,
    )
    if profile_json_emittable != computed_profile_json_emittable:
        raise EvidenceError(
            f"{path}.profile_json_emittable does not match trust source policy"
        )
    if profile_json_emitted and not profile_json_emittable:
        raise EvidenceError(
            f"{path}.profile_json_emitted cannot be true when profile_json_emittable is false"
        )
    if profile_json_emitted and not computed_profile_json_emittable:
        raise EvidenceError(
            f"{path}.profile_json_emitted cannot be true when trust source policy is not emittable"
        )
    if profile_json_emitted:
        profile_config = [bundle["profile_overrides"] for bundle in bundle_objects]
        expected_profile_text = json.dumps(profile_config, indent=2, sort_keys=True) + "\n"
        expected_profile_sha256 = sha256_hex(expected_profile_text.encode("utf-8"))
        if profile_json_sha256 != expected_profile_sha256:
            raise EvidenceError(
                f"{path}.profile_json_sha256 does not match archived profile_overrides"
            )
    seen_profile_ids: dict[str, int] = {}
    seen_bundle_digests: dict[str, int] = {}
    for offset, bundle in enumerate(bundle_summaries):
        profile_id = bundle["profile_id"]
        if profile_id in seen_profile_ids:
            raise EvidenceError(
                f"{path}.bundles[{offset}].profile_id duplicates "
                f"{path}.bundles[{seen_profile_ids[profile_id]}].profile_id"
            )
        seen_profile_ids[profile_id] = offset
        bundle_sha256 = bundle["bundle_sha256"]
        if bundle_sha256 in seen_bundle_digests:
            raise EvidenceError(
                f"{path}.bundles[{offset}].bundle_sha256 duplicates "
                f"{path}.bundles[{seen_bundle_digests[bundle_sha256]}].bundle_sha256"
            )
        seen_bundle_digests[bundle_sha256] = offset
    return {
        "version": version,
        "path": str(path),
        "verified_at": verified_at_raw,
        "verified_bundles": verified_bundles,
        "max_source_age_days": max_source_age_days,
        "allow_synthetic_der": allow_synthetic_der,
        "allow_record_only": allow_record_only,
        "allow_insecure_source_url": allow_insecure_source_url,
        "profile_json_emitted": profile_json_emitted,
        "profile_json_emittable": profile_json_emittable,
        "profile_json_sha256": profile_json_sha256,
        "profiles": bundle_summaries,
        "summary_sha256": digest,
    }


def verify_receipts(args: argparse.Namespace) -> dict[str, Any] | None:
    """Optionally invoke the existing receipt verifier in read-only mode."""

    if not args.receipt and not args.receipt_dir:
        return None
    command = [sys.executable, str(SCRIPT_DIR / "iso_operator_receipt_verify.py")]
    for receipt in args.receipt:
        command.extend(["--receipt", str(receipt)])
    for receipt_dir in args.receipt_dir:
        command.extend(["--receipt-dir", str(receipt_dir)])
    if args.allow_failed_receipts:
        command.append("--allow-failed")
    if args.allow_insecure_http:
        command.append("--allow-insecure-http")
    if args.allow_legacy_colr007:
        command.append("--allow-legacy-colr007")
    if args.allow_default_profile:
        command.append("--allow-default-profile")
    if not args.allow_receipt_source_missing:
        command.append("--require-source-files")
    (
        returncode,
        stdout,
        stdout_truncated,
        stderr,
        stderr_truncated,
        timed_out,
    ) = _run_command_bounded(
        command,
        MAX_RECEIPT_VERIFIER_OUTPUT_BYTES,
        args.receipt_verifier_timeout_secs,
    )
    if timed_out:
        raise EvidenceError(
            "receipt verifier timed out after "
            f"{args.receipt_verifier_timeout_secs:g} seconds"
        )
    if returncode != 0:
        detail = stderr.strip()[:4096]
        if detail and (
            _contains_secret_material(detail)
            or _contains_secret_identifier_material(detail)
        ):
            detail = "[receipt verifier stderr redacted: secret-looking material]"
        if stderr_truncated:
            detail = (
                f"{detail} [stderr truncated at "
                f"{MAX_RECEIPT_VERIFIER_OUTPUT_BYTES} bytes]"
            )
        raise EvidenceError("receipt verification failed: " + detail)
    if stdout_truncated:
        raise EvidenceError(
            "receipt verifier stdout exceeded "
            f"{MAX_RECEIPT_VERIFIER_OUTPUT_BYTES} byte limit"
        )
    if stderr_truncated:
        raise EvidenceError(
            "receipt verifier stderr exceeded "
            f"{MAX_RECEIPT_VERIFIER_OUTPUT_BYTES} byte limit"
        )
    try:
        receipt_summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError("receipt verifier emitted invalid JSON") from error
    _reject_json_surrogates(receipt_summary)
    receipt_obj = _require_object(receipt_summary, "receipt verifier summary")
    return _verify_receipt_verifier_summary(receipt_obj, "receipt verifier summary", args)


def _verify_direct_receipts_cover_canaries(
    canaries: list[dict[str, Any]],
    receipt_summary: dict[str, Any],
) -> None:
    """Require direct receipt archive verification to match canary receipt digests."""

    direct_receipts_by_digest = {
        receipt["receipt_sha256"]: receipt
        for receipt in receipt_summary["receipts"]
    }
    canary_receipt_kinds_by_digest: dict[str, str] = {}
    for canary_offset, canary in enumerate(canaries):
        canary_receipt_summary = canary.get("receipt_summary")
        if canary_receipt_summary is None:
            continue
        for receipt_offset, receipt in enumerate(canary_receipt_summary["receipts"]):
            receipt_sha256 = receipt["receipt_sha256"]
            canary_receipt_kinds_by_digest[receipt_sha256] = receipt["receipt_kind"]
            direct_receipt = direct_receipts_by_digest.get(receipt_sha256)
            if direct_receipt is None:
                raise EvidenceError(
                    "direct receipt archive verification does not include "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 {receipt_sha256}"
                )
            direct_kind = direct_receipt["receipt_kind"]
            if direct_kind != receipt["receipt_kind"]:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 {receipt_sha256} to "
                    f"receipt_kind {direct_kind!r}, not {receipt['receipt_kind']!r}"
                )
            direct_path_name = Path(direct_receipt["path"]).name
            canary_path_name = Path(receipt["path"]).name
            if direct_path_name != canary_path_name:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 {receipt_sha256} to "
                    f"receipt filename {direct_path_name!r}, not {canary_path_name!r}"
                )
            direct_metadata = _receipt_entry_content_metadata(direct_receipt)
            canary_metadata = _receipt_entry_content_metadata(receipt)
            if direct_metadata != canary_metadata:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 {receipt_sha256} to "
                    f"metadata {direct_metadata!r}, not {canary_metadata!r}"
                )
    for receipt_offset, receipt in enumerate(receipt_summary["receipts"]):
        receipt_sha256 = receipt["receipt_sha256"]
        if receipt_sha256 not in canary_receipt_kinds_by_digest:
            raise EvidenceError(
                "direct receipt archive verification includes unreferenced "
                f"receipt_verification.receipts[{receipt_offset}].receipt_sha256 "
                f"{receipt_sha256}"
            )


def _reject_cross_canary_receipt_reuse(canaries: list[dict[str, Any]]) -> None:
    """Reject receipt path or digest reuse across distinct canary summaries."""

    seen_paths: dict[str, tuple[int, int]] = {}
    seen_digests: dict[str, tuple[int, int]] = {}
    for canary_offset, canary in enumerate(canaries):
        receipt_summary = canary.get("receipt_summary")
        if receipt_summary is None:
            continue
        for receipt_offset, receipt in enumerate(receipt_summary["receipts"]):
            receipt_path = receipt["path"]
            if receipt_path in seen_paths:
                first_canary, first_receipt = seen_paths[receipt_path]
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].path duplicates "
                    f"canary_summaries[{first_canary}].receipt_summary.receipts"
                    f"[{first_receipt}].path"
                )
            seen_paths[receipt_path] = (canary_offset, receipt_offset)
            receipt_sha256 = receipt["receipt_sha256"]
            if receipt_sha256 in seen_digests:
                first_canary, first_receipt = seen_digests[receipt_sha256]
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 duplicates "
                    f"canary_summaries[{first_canary}].receipt_summary.receipts"
                    f"[{first_receipt}].receipt_sha256"
                )
            seen_digests[receipt_sha256] = (canary_offset, receipt_offset)


def _reject_cross_trust_profile_reuse(trusts: list[dict[str, Any]]) -> None:
    """Reject trust profile material reused across distinct trust summaries."""

    seen_profile_ids: dict[str, tuple[int, int]] = {}
    seen_bundle_digests: dict[str, tuple[int, int]] = {}
    for trust_offset, trust in enumerate(trusts):
        for profile_offset, profile in enumerate(trust["profiles"]):
            profile_id = profile["profile_id"]
            if profile_id in seen_profile_ids:
                first_trust, first_profile = seen_profile_ids[profile_id]
                raise EvidenceError(
                    f"trust_summaries[{trust_offset}].profiles[{profile_offset}].profile_id "
                    f"duplicates trust_summaries[{first_trust}].profiles"
                    f"[{first_profile}].profile_id"
                )
            seen_profile_ids[profile_id] = (trust_offset, profile_offset)
            bundle_sha256 = profile["bundle_sha256"]
            if bundle_sha256 in seen_bundle_digests:
                first_trust, first_profile = seen_bundle_digests[bundle_sha256]
                raise EvidenceError(
                    f"trust_summaries[{trust_offset}].profiles[{profile_offset}].bundle_sha256 "
                    f"duplicates trust_summaries[{first_trust}].profiles"
                    f"[{first_profile}].bundle_sha256"
                )
            seen_bundle_digests[bundle_sha256] = (trust_offset, profile_offset)


def _reject_canary_rail_receipts_without_trust(
    canaries: list[dict[str, Any]],
    trusts: list[dict[str, Any]],
    args: argparse.Namespace,
) -> None:
    """Reject canary rail receipts that lack matching trust material."""

    trusted_profiles_by_environment = {
        (profile["profile_id"], profile["environment"])
        for trust in trusts
        for profile in trust["profiles"]
    }
    trusted_builtin_profiles_by_rail_environment = {
        (profile["profile_id"], profile["rail"], profile["environment"])
        for trust in trusts
        for profile in trust["profiles"]
    }
    for canary_offset, canary in enumerate(canaries):
        receipt_summary = canary.get("receipt_summary")
        if receipt_summary is None:
            continue
        canary_environment = canary["environment"]
        for receipt_offset, receipt in enumerate(receipt_summary["receipts"]):
            if receipt["receipt_kind"] != "iso-rail-gateway":
                continue
            if (
                args.allow_legacy_colr007
                and receipt.get("message_type") in LEGACY_RAIL_MESSAGE_TYPES
            ):
                continue
            profile_id = receipt["profile"]
            if profile_id is None and args.allow_default_profile:
                continue
            if profile_id in KNOWN_RAILS:
                covered = (
                    profile_id,
                    profile_id,
                    canary_environment,
                ) in trusted_builtin_profiles_by_rail_environment
            else:
                covered = (
                    profile_id,
                    canary_environment,
                ) in trusted_profiles_by_environment
            if not covered:
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].profile {profile_id!r} has no matching "
                    f"trust profile coverage for environment {canary_environment!r}"
                )


def run(args: argparse.Namespace) -> int:
    if not args.canary_summary:
        raise EvidenceError("provide at least one --canary-summary")
    if not args.trust_summary:
        raise EvidenceError("provide at least one --trust-summary")
    args.provider = _required_cli_string(args.provider, "--provider")
    args.environment = _required_cli_string(args.environment, "--environment")
    args.max_canary_age_days = _required_positive_cli_int(
        args.max_canary_age_days,
        "--max-canary-age-days",
    )
    args.max_trust_age_days = _required_positive_cli_int(
        args.max_trust_age_days,
        "--max-trust-age-days",
    )
    args.max_trust_source_age_days = _required_positive_cli_int(
        args.max_trust_source_age_days,
        "--max-trust-source-age-days",
    )
    args.receipt_verifier_timeout_secs = _required_positive_finite_cli_number(
        args.receipt_verifier_timeout_secs,
        "--receipt-verifier-timeout-secs",
    )

    canary_paths = list(args.canary_summary)
    trust_paths = list(args.trust_summary)
    _reject_duplicate_paths([path.resolve() for path in canary_paths], "--canary-summary")
    _reject_duplicate_paths([path.resolve() for path in trust_paths], "--trust-summary")

    canaries = [verify_canary_summary(path, args) for path in canary_paths]
    trusts = [verify_trust_summary(path, args) for path in trust_paths]
    _reject_duplicate_summary_digests(canaries, "canary_summaries")
    _reject_duplicate_summary_digests(trusts, "trust_summaries")
    _reject_cross_canary_receipt_reuse(canaries)
    _reject_cross_trust_profile_reuse(trusts)
    receipt_summary = verify_receipts(args)
    if receipt_summary is None and not args.allow_canary_stage_receipts_only:
        raise EvidenceError(
            "provide --receipt or --receipt-dir for direct receipt archive verification"
        )
    if receipt_summary is not None:
        _verify_direct_receipts_cover_canaries(canaries, receipt_summary)
    _reject_canary_rail_receipts_without_trust(canaries, trusts, args)

    output: dict[str, Any] = {
        "version": EVIDENCE_VERSION,
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": True,
        "canary_summaries": canaries,
        "trust_summaries": trusts,
        "receipt_verification": receipt_summary,
        "policy": {
            "provider": args.provider,
            "environment": args.environment,
            "allow_plan_only": args.allow_plan_only,
            "allow_dry_run": args.allow_dry_run,
            "allow_insecure_http": args.allow_insecure_http,
            "allow_legacy_colr007": args.allow_legacy_colr007,
            "allow_default_profile": args.allow_default_profile,
            "allow_failed_receipts": args.allow_failed_receipts,
            "allow_partial_canary": args.allow_partial_canary,
            "allow_canary_stage_receipts_only": args.allow_canary_stage_receipts_only,
            "allow_receipt_source_missing": args.allow_receipt_source_missing,
            "allow_record_only_trust": args.allow_record_only_trust,
            "allow_synthetic_trust": args.allow_synthetic_trust,
            "allow_missing_trust_source": args.allow_missing_trust_source,
            "allow_profile_json_not_emitted": args.allow_profile_json_not_emitted,
            "max_canary_age_days": args.max_canary_age_days,
            "max_trust_age_days": args.max_trust_age_days,
            "max_trust_source_age_days": args.max_trust_source_age_days,
        },
    }
    output[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(output))
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        _write_text_output(args.summary_out, text)
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify archived ISO 20022 operator canary and trust evidence."
    )
    parser.add_argument(
        "--canary-summary",
        action="append",
        default=[],
        type=Path,
        help="Canary summary JSON produced by iso_operator_canary.py; repeatable.",
    )
    parser.add_argument(
        "--trust-summary",
        action="append",
        default=[],
        type=Path,
        help="Trust summary JSON produced by iso_trust_bundle_verify.py; repeatable.",
    )
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        type=Path,
        help="Optional receipt JSON file to re-verify; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        action="append",
        default=[],
        type=Path,
        help="Optional directory of *.receipt.json files to re-verify; repeatable.",
    )
    parser.add_argument(
        "--provider",
        help="Expected canary provider value.",
    )
    parser.add_argument(
        "--environment",
        help="Expected canary and trust environment value.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the evidence verification summary JSON.",
    )
    parser.add_argument(
        "--max-canary-age-days",
        type=int,
        help="Maximum age in days for canary finished_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-age-days",
        type=int,
        help="Maximum age in days for trust-bundle summary verified_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-source-age-days",
        type=int,
        help="Maximum age in days for trust source retrieved_at timestamps.",
    )
    parser.add_argument(
        "--allow-plan-only",
        action="store_true",
        help="Allow plan-only canary summaries for local dry audits.",
    )
    parser.add_argument(
        "--allow-dry-run",
        action="store_true",
        help="Allow child canary commands that include --dry-run.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// URLs and insecure HTTP overrides for local tests.",
    )
    parser.add_argument(
        "--allow-default-profile",
        action="store_true",
        help="Allow rail gateway canaries that use --allow-default-profile.",
    )
    parser.add_argument(
        "--allow-failed-receipts",
        action="store_true",
        help="Allow receipt verifier runs configured with --allow-failed.",
    )
    parser.add_argument(
        "--allow-legacy-colr007",
        action="store_true",
        help="Allow local diagnostic evidence that used legacy colr.007 rail receipts.",
    )
    parser.add_argument(
        "--allow-partial-canary",
        action="store_true",
        help="Allow canaries with only rail or only notary plus verify.",
    )
    parser.add_argument(
        "--allow-canary-stage-receipts-only",
        action="store_true",
        help="Allow summaries without direct receipt archive verification for local audits.",
    )
    parser.add_argument(
        "--allow-receipt-source-missing",
        action="store_true",
        help="Do not require receipt verifier commands to use --require-source-files.",
    )
    parser.add_argument(
        "--receipt-verifier-timeout-secs",
        type=float,
        default=DEFAULT_RECEIPT_VERIFIER_TIMEOUT_SECS,
        help="Maximum wall-clock seconds allowed for direct receipt archive verification.",
    )
    parser.add_argument(
        "--allow-record-only-trust",
        action="store_true",
        help="Allow trust summaries produced with record-only signature policy.",
    )
    parser.add_argument(
        "--allow-synthetic-trust",
        action="store_true",
        help="Allow trust summaries produced with --allow-synthetic-der.",
    )
    parser.add_argument(
        "--allow-missing-trust-source",
        action="store_true",
        help="Allow trust bundle summaries without provenance source metadata.",
    )
    parser.add_argument(
        "--allow-profile-json-not-emitted",
        action="store_true",
        help="Allow trust summaries that did not emit profile override JSON for local audits.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        _preflight_raw_cli_secrets(
            argv,
            {
                "--canary-summary",
                "--max-canary-age-days",
                "--max-trust-age-days",
                "--max-trust-source-age-days",
                "--receipt",
                "--receipt-dir",
                "--receipt-verifier-timeout-secs",
                "--summary-out",
                "--trust-summary",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--allow-canary-stage-receipts-only",
                "--allow-default-profile",
                "--allow-dry-run",
                "--allow-failed-receipts",
                "--allow-insecure-http",
                "--allow-legacy-colr007",
                "--allow-missing-trust-source",
                "--allow-partial-canary",
                "--allow-plan-only",
                "--allow-profile-json-not-emitted",
                "--allow-receipt-source-missing",
                "--allow-record-only-trust",
                "--allow-synthetic-trust",
            },
        )
        _preflight_required_cli_values(
            argv,
            {"--environment", "--provider"},
            "context",
        )
        _preflight_numeric_cli_values(
            argv,
            integer_flags={
                "--max-canary-age-days",
                "--max-trust-age-days",
                "--max-trust-source-age-days",
            },
            number_flags={"--receipt-verifier-timeout-secs"},
        )
        _preflight_output_cli_paths(
            argv,
            {
                "--canary-summary",
                "--receipt",
                "--receipt-dir",
                "--summary-out",
                "--trust-summary",
            },
        )
        args = parser.parse_args(argv)
        return run(args)
    except EvidenceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
