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
import unicodedata
import urllib.parse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


EVIDENCE_VERSION = 1
CANARY_SUMMARY_VERSION = 1
RECEIPT_SUMMARY_VERSION = 2
TRUST_SUMMARY_VERSION = 1
REQUIRE_VERIFIED = "require-verified"
TRUST_SIGNATURE_POLICIES = {"record-only", "reject-unsupported", REQUIRE_VERIFIED}
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.[0-9]{3}$")
RAIL_MESSAGE_ID_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._:@+-]*[A-Za-z0-9])?$")
CLI_CANONICAL_INT_RE = re.compile(r"(?:0|-?[1-9][0-9]*)")
JSON_CANONICAL_INT_RE = re.compile(r"(?:0|-?[1-9][0-9]*)")
CLI_CANONICAL_NUMBER_RE = re.compile(
    r"-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?(?:0|[1-9][0-9]*))?"
)
CANONICAL_TIMESTAMP_RE = re.compile(
    r"[0-9]{4}-[0-9]{2}-[0-9]{2}T"
    r"[0-9]{2}:[0-9]{2}:[0-9]{2}"
    r"(?:\.[0-9]{1,6})?"
    r"(?:Z|[+-][0-9]{2}:[0-9]{2})"
)
CLI_NONFINITE_NUMBER_TEXTS = {
    "inf",
    "+inf",
    "-inf",
    "infinity",
    "+infinity",
    "-infinity",
    "nan",
    "+nan",
    "-nan",
}


def _is_negative_zero_number_text(value: str) -> bool:
    if not value.startswith("-"):
        return False
    mantissa = re.split(r"[eE]", value[1:], maxsplit=1)[0]
    return all(ch in {"0", "."} for ch in mantissa)
LOCAL_PATH_ERROR_RE = re.compile(
    r"(?:^|[\s'\"(<:=])(?:[A-Za-z]:[\\/]|/[^/\s'\"<>]+|\.{1,2}/|~/)"
)
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
RECEIPT_SOURCE_MATERIAL_FIELDS_BY_KIND = {
    "iso-audit-notary": (
        "anchor_path",
        "anchor_sha256",
        "index_path",
        "index_sha256",
    ),
    "iso-rail-gateway": ("source_path", "payload_sha256", "rail_message_id"),
}
STAGE_RECEIPT_KINDS = {
    "rail": "iso-rail-gateway",
    "notary": "iso-audit-notary",
}
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
RAIL_MESSAGE_TYPES = {
    "generic-iso20022": SUPPORTED_RAIL_MESSAGE_TYPES - LEGACY_RAIL_MESSAGE_TYPES,
    "swift-cbpr-plus": {
        "pacs.008",
        "pacs.009",
        "pacs.002",
        "pacs.004",
        "camt.056",
    },
    "fedwire-funds": {
        "pacs.008",
        "pacs.009",
        "pacs.002",
        "pacs.004",
        "camt.056",
    },
    "sepa-sct-inst": {
        "pacs.008",
        "pacs.002",
        "pacs.004",
        "camt.056",
    },
    "securities-csd": {
        "pacs.009",
        "pacs.002",
        "pacs.004",
        "camt.056",
        "sese.023",
        "sese.024",
        "sese.025",
        "colr.012",
    },
}
MAX_TRUST_DER_BLOBS = 8
MAX_TRUST_DER_BYTES = 1024 * 1024
MAX_TRUST_DER_BASE64_CHARS = ((MAX_TRUST_DER_BYTES + 2) // 3) * 4
TRUST_DER_KIND_CRL = "X.509 CRL"
TRUST_DER_KIND_OCSP = "OCSPResponse"
OID_OCSP_BASIC_RESPONSE_DER = b"\x2b\x06\x01\x05\x05\x07\x30\x01\x01"
MAX_PROFILE_ID_CHARS = 128
MAX_TRUST_POLICY_CHARS = 128
MAX_TRUST_SOURCE_TEXT_CHARS = 256
MAX_RAIL_MESSAGE_ID_CHARS = 128
MAX_TIMESTAMP_CHARS = 128
MAX_SUMMARY_JSON_BYTES = 4 * 1024 * 1024
MAX_RECEIPT_VERIFIER_OUTPUT_BYTES = 4 * 1024 * 1024
MAX_EVIDENCE_INPUT_PATHS = 64
MAX_JSON_LIST_ITEMS = 8192
MAX_JSON_OBJECT_MEMBERS = 8192
MAX_JSON_NESTING_DEPTH = 128
MAX_HTTP_URL_CHARS = 2048
MAX_LOCAL_PATH_CHARS = 4096
MAX_CLEAN_STRING_CHARS = 4096
ANCHOR_DIR = "anchors"
INDEX_FILE = "messages.index.json"
LATEST_ANCHOR_FILE = "latest.notary.json"
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
REPOSITORY_CANARY_RUNBOOK_PARTS = (
    "fixtures",
    "iso20022",
    "operator_canary",
)
REPOSITORY_TRUST_BUNDLE_PARTS = (
    "fixtures",
    "iso20022",
    "trust_bundles",
)
REPOSITORY_XML_FIXTURE_PARTS = (
    "fixtures",
    "iso20022",
)
LOCAL_REBINDING_HOST_SUFFIXES = {"localtest.me", "lvh.me", "nip.io", "sslip.io", "vcap.me"}
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
SCRIPT_DIR = Path(__file__).resolve().parent


@dataclass(frozen=True)
class DerElement:
    """One parsed DER TLV element used for lightweight trust-material replay checks."""

    tag: int
    header_len: int
    length: int
    start: int
    value_start: int
    end: int
    value: bytes

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
RAIL_STAGE_STDOUT_KEYS = {"submitted_messages", "receipts", "failures"}
NOTARY_STAGE_STDOUT_KEYS = {"published_anchors", "endpoint_count", "receipts", "failures"}
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
    "response_body_sha256",
    "endpoint_requires_insecure_http",
    "anchor_path",
    "store_dir",
    "index_path",
    "anchor_sha256",
    "index_sha256",
    "record_count",
    "message_type",
    "payload_sha256",
    "profile",
    "rail_message_id",
    "source_path",
}
NOTARY_RECEIPT_METADATA_KEYS = {
    "anchor_path",
    "store_dir",
    "index_path",
    "anchor_sha256",
    "index_sha256",
    "record_count",
}
RAIL_RECEIPT_METADATA_KEYS = {
    "message_type",
    "payload_sha256",
    "profile",
    "rail_message_id",
    "source_path",
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
    "private key",
    "private.key",
    "privatekey",
    "password",
    "passphrase",
    "api_key",
    "api-key",
    "api key",
    "api.key",
    "apikey",
    "access_key",
    "access-key",
    "access key",
    "access.key",
    "accesskey",
    "session_key",
    "session-key",
    "session key",
    "session.key",
    "sessionkey",
    "client_secret",
    "client-secret",
    "client secret",
    "client.secret",
    "clientsecret",
    "cookie",
    "set-cookie",
    "set cookie",
    "set.cookie",
    "setcookie",
    "x-iroha-signature",
    "x_iroha_signature",
    "x iroha signature",
    "x.iroha.signature",
    "xirohasignature",
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
        r"\b(?:token|secret|private[\s_./\\-]*key|password|passphrase|api[\s_./\\-]*key|access[\s_./\\-]*key|session[\s_./\\-]*key|client[\s_./\\-]*secret|cookie|set[\s_./\\-]*cookie)\s*[:=]\s*\S+",
        re.IGNORECASE,
    ),
    re.compile(r"\bx[\s_./\\-]*iroha[\s_./\\-]*signature\s*:", re.IGNORECASE),
]


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
        for raw_candidate in _secret_scan_values(value)
        for candidate in _secret_value_forms(raw_candidate)
        for pattern in SECRET_VALUE_PATTERNS
    )


def _secret_value_forms(value: str) -> tuple[str, ...]:
    return _secret_base_forms(value)


def _secret_base_forms(value: str) -> tuple[str, ...]:
    folded = value.casefold()
    forms: list[str] = []
    for candidate in (
        folded,
        unicodedata.normalize("NFKC", folded).casefold(),
        unicodedata.normalize("NFKD", folded).casefold(),
    ):
        without_obfuscation = "".join(
            ch for ch in candidate if not _is_secret_obfuscation_char(ch)
        )
        obfuscation_spaced = "".join(
            " " if _is_secret_obfuscation_char(ch) else ch for ch in candidate
        )
        forms.extend((candidate, without_obfuscation, obfuscation_spaced))
    return tuple(dict.fromkeys(forms))


def _is_secret_obfuscation_char(ch: str) -> bool:
    category = unicodedata.category(ch)
    return category == "Cf" or category.startswith("M")


def _secret_identifier_forms(value: str) -> tuple[str, ...]:
    forms: list[str] = []
    for candidate in _secret_base_forms(value):
        forms.extend(
            (
                candidate,
                re.sub(r"[\s_./\\-]+", " ", candidate).strip(),
                re.sub(r"[\s_./\\-]+", "", candidate),
            )
        )
    return tuple(dict.fromkeys(forms))


def _contains_secret_marker(value: str, markers: tuple[str, ...]) -> bool:
    candidate_forms = _secret_identifier_forms(value)
    return any(
        marker_form in candidate_form
        for marker in markers
        for marker_form in _secret_identifier_forms(marker)
        for candidate_form in candidate_forms
    )


def _contains_secret_identifier_material(value: str) -> bool:
    strong_markers = (
        "private_key",
        "private-key",
        "private key",
        "private.key",
        "privatekey",
        "password",
        "passphrase",
        "api_key",
        "api-key",
        "api key",
        "api.key",
        "apikey",
        "access_key",
        "access-key",
        "access key",
        "access.key",
        "accesskey",
        "session_key",
        "session-key",
        "session key",
        "session.key",
        "sessionkey",
        "client_secret",
        "client-secret",
        "client secret",
        "client.secret",
        "clientsecret",
        "set-cookie",
        "set cookie",
        "set.cookie",
        "setcookie",
        "x-iroha-signature",
        "x_iroha_signature",
        "x iroha signature",
        "x.iroha.signature",
        "xirohasignature",
    )
    paired_markers = ("authorization", "bearer", "token", "cookie")
    return any(
        _contains_secret_marker(candidate, strong_markers)
        or (
            _contains_secret_marker(candidate, ("secret",))
            and _contains_secret_marker(candidate, paired_markers)
        )
        for candidate in _secret_scan_values(value)
    )


def _receipt_verifier_stderr_detail(stderr: str) -> str:
    detail = stderr.strip()
    if not detail:
        return ""
    if _contains_secret_material(detail) or _contains_secret_identifier_material(detail):
        return "[receipt verifier stderr redacted: secret-looking material]"
    if not detail.isascii():
        return "[receipt verifier stderr redacted: non-ASCII material]"
    if _contains_local_path_material(detail):
        return "[receipt verifier stderr redacted: local path material]"
    if _contains_unsafe_diagnostic_control(detail):
        return "[receipt verifier stderr redacted: control characters]"
    return _single_line_diagnostic_detail(detail)[:4096]


def _single_line_diagnostic_detail(value: str) -> str:
    return " ".join(value.split())


def _contains_local_path_material(value: str) -> bool:
    return (
        "\\" in value
        or "file:" in value.casefold()
        or LOCAL_PATH_ERROR_RE.search(value) is not None
    )


def _contains_unsafe_diagnostic_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _contains_unsafe_preview_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\r", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _contains_control_character(value: str) -> bool:
    return any(
        ord(ch) < 0x20 or ord(ch) == 0x7F or unicodedata.category(ch) == "Cf"
        for ch in value
    )


class EvidenceError(RuntimeError):
    """Raised when archived ISO operator evidence is unsafe or malformed."""


def _plain_text(value: str, label: str) -> str:
    try:
        return str.__str__(value)
    except Exception:
        raise EvidenceError(f"{label} must be valid text") from None


def _normalise_cli_argv(argv: list[str] | None) -> list[str]:
    if argv is None:
        raw_sys_argv = sys.argv
        if type(raw_sys_argv) is not list:
            raise EvidenceError("sys.argv must be a plain argument list")
        raw_args = raw_sys_argv[1:]
    else:
        raw_args = argv
    if type(raw_args) is not list:
        raise EvidenceError("argv must be a plain argument list")
    normalised: list[str] = []
    for index, value in enumerate(raw_args):
        if not isinstance(value, str):
            raise EvidenceError(f"argv[{index}] must be a string")
        normalised.append(_plain_text(value, f"argv[{index}]"))
    return normalised


def _require_plain_namespace(args: argparse.Namespace) -> argparse.Namespace:
    if type(args) is not argparse.Namespace:
        raise EvidenceError("args must be an argparse.Namespace")
    return args


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        allow_nan=False,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _safe_os_error_detail(error: OSError) -> str:
    try:
        detail = getattr(error, "strerror", None)
    except Exception:
        return "I/O error"
    if not isinstance(detail, str) or not detail.strip():
        return "I/O error"
    if len(detail) > 128 or not detail.isascii() or _contains_control_character(detail):
        return "I/O error"
    lowered = detail.casefold()
    secret_markers = (
        "authorization",
        "bearer",
        "token",
        "secret",
        "private",
        "password",
        "passphrase",
        "api key",
        "access key",
        "session key",
        "client secret",
        "cookie",
        "x-iroha-signature",
    )
    if "\\" in detail or "/" in detail or "file:" in lowered:
        return "I/O error"
    if any(marker in lowered for marker in secret_markers):
        return "I/O error"
    return detail


def _read_regular_file(
    path: Path,
    *,
    max_bytes: int | None = None,
    display_label: str | None = None,
) -> bytes:
    label = display_label or str(path)
    if max_bytes is not None and (
        isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or max_bytes <= 0
    ):
        raise EvidenceError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise EvidenceError(f"{label} does not exist") from error
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise EvidenceError(f"cannot inspect {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise EvidenceError(f"cannot inspect {label}: I/O error") from None
    mode = metadata.st_mode
    if stat.S_ISLNK(mode):
        raise EvidenceError(f"{label} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise EvidenceError(f"{label} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise EvidenceError(f"{label} exceeds {max_bytes} byte JSON limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise EvidenceError(f"{label} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise EvidenceError(f"{label} exceeds {max_bytes} byte JSON limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise EvidenceError(f"{label} exceeds {max_bytes} byte JSON limit")
        return raw
    except FileNotFoundError as error:
        raise EvidenceError(f"{label} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise EvidenceError(f"{label} must not be a symlink") from error
        detail = _safe_os_error_detail(error)
        raise EvidenceError(f"cannot open {label} for reading: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise EvidenceError(f"cannot open {label} for reading: I/O error") from None
    finally:
        if fd >= 0:
            try:
                os.close(fd)
            except (OSError, RuntimeError, TypeError, ValueError):
                pass


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise EvidenceError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise EvidenceError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
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
    if ":" in raw:
        raise EvidenceError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise EvidenceError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise EvidenceError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise EvidenceError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise EvidenceError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
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
    if ":" in raw:
        raise EvidenceError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise EvidenceError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise EvidenceError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise EvidenceError(f"{label} must not contain dot or parent segments")


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise EvidenceError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise EvidenceError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise EvidenceError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise EvidenceError(f"{label} must not contain encoded semicolon parameters")
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise EvidenceError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise EvidenceError(f"{label} must not contain encoded percent characters")
        index += 3


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise EvidenceError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if _contains_control_character(arg):
            raise EvidenceError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise EvidenceError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise EvidenceError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise EvidenceError("argument terminator is not supported")
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


def _reject_raw_context_cli_value(raw: str, flag: str) -> None:
    if _contains_control_character(raw):
        raise EvidenceError(f"{flag} must not contain control characters")
    if not raw.strip():
        return
    if raw != raw.strip():
        raise EvidenceError(f"{flag} must not have surrounding whitespace")
    if any(ord(ch) > 0x7E for ch in raw):
        raise EvidenceError(f"{flag} must use printable ASCII")
    _reject_secret_looking_identifier(raw, flag)


def _reject_raw_profile_cli_value(raw: str, flag: str) -> None:
    if _contains_control_character(raw):
        raise EvidenceError(f"{flag} must not contain control characters")
    if not raw.strip():
        return
    if raw != raw.strip():
        raise EvidenceError(f"{flag} must not have surrounding whitespace")
    if any(ord(ch) > 0x7E for ch in raw):
        raise EvidenceError(f"{flag} must use printable ASCII")
    if len(raw) > MAX_PROFILE_ID_CHARS:
        raise EvidenceError(
            f"{flag} must be no longer than {MAX_PROFILE_ID_CHARS} characters"
        )
    if PROFILE_ID_RE.fullmatch(raw) is None:
        raise EvidenceError(f"{flag} must be a canonical lowercase profile id")
    _reject_secret_looking_identifier(raw, flag)


def _preflight_required_cli_values(
    argv: list[str] | None,
    flags: set[str],
    value_name: str,
) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise EvidenceError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                value = raw_args[index + 1]
                if not value or value.startswith("-"):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                if value_name == "context":
                    _reject_raw_context_cli_value(value, flag)
                elif value_name == "profile id":
                    _reject_raw_profile_cli_value(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("-"):
                    raise EvidenceError(f"{flag} requires a {value_name} value")
                if value_name == "context":
                    _reject_raw_context_cli_value(value, flag)
                elif value_name == "profile id":
                    _reject_raw_profile_cli_value(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _preflight_output_cli_paths(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise EvidenceError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise EvidenceError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                _reject_repository_artifact_path(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise EvidenceError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                _reject_repository_artifact_path(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or _contains_control_character(raw):
        raise EvidenceError(f"{flag} must be a numeric value")
    if any(ord(ch) > 0x7E for ch in raw):
        raise EvidenceError(f"{flag} must use printable ASCII")
    if integer:
        canonical = CLI_CANONICAL_INT_RE.fullmatch(raw) is not None
    else:
        canonical = (
            raw.lower() in CLI_NONFINITE_NUMBER_TEXTS
            or CLI_CANONICAL_NUMBER_RE.fullmatch(raw) is not None
        )
    if not canonical or _is_negative_zero_number_text(raw):
        raise EvidenceError(f"{flag} must be a numeric value")
    try:
        parsed = int(raw, 10) if integer else float(raw)
    except ValueError as error:
        raise EvidenceError(f"{flag} must be a numeric value") from error
    if (
        not integer
        and raw.lower() not in CLI_NONFINITE_NUMBER_TEXTS
        and not math.isfinite(parsed)
    ):
        raise EvidenceError(f"{flag} must be a numeric value")


def _preflight_numeric_cli_values(
    argv: list[str] | None,
    *,
    integer_flags: set[str],
    number_flags: set[str],
) -> None:
    raw_args = _normalise_cli_argv(argv)
    flags = integer_flags | number_flags
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise EvidenceError("argument terminator is not supported")
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


def _reject_repository_artifact_path(path: str | Path, label: str) -> None:
    if _receipt_path_is_repository_fixture(str(path)):
        raise EvidenceError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )


def _reject_repository_output_path(path: Path, label: str) -> None:
    _reject_repository_artifact_path(path, label)


def _same_existing_file(left: Path, right: Path) -> bool:
    try:
        left_stat = left.stat()
        right_stat = right.stat()
    except FileNotFoundError:
        return False
    except (OSError, RuntimeError, TypeError, ValueError):
        return False
    return os.path.samestat(left_stat, right_stat)


def _path_resolve(path: Path, label: str) -> Path:
    try:
        return path.resolve()
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise EvidenceError(f"cannot resolve {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise EvidenceError(f"cannot resolve {label}: I/O error") from None


def _reject_summary_output_input_alias(
    summary_out: Path | None,
    inputs: tuple[tuple[str, Path], ...],
) -> None:
    if summary_out is None:
        return
    for label, path in inputs:
        if str(summary_out) == str(path) or _same_existing_file(summary_out, path):
            raise EvidenceError(f"summary_out must not reuse {label} path")


def _reject_summary_output_receipt_dir_overlap(
    summary_out: Path | None,
    receipt_dirs: tuple[Path, ...],
) -> None:
    if summary_out is None:
        return
    summary_path = _path_resolve(summary_out, "summary_out")
    for offset, receipt_dir in enumerate(receipt_dirs):
        receipt_root = _path_resolve(receipt_dir, f"--receipt-dir[{offset}]")
        if summary_path == receipt_root or receipt_root in summary_path.parents:
            raise EvidenceError(
                f"summary_out must not be written under --receipt-dir[{offset}]"
            )


def _ensure_text_output_target(
    path: Path,
    *,
    display_label: str | None = None,
    create_parent: bool = True,
) -> None:
    label = display_label if display_label is not None else "output path"
    _reject_output_path_smuggling(path, label)
    _reject_repository_output_path(path, label)
    try:
        _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    except NotADirectoryError as error:
        raise EvidenceError(f"{label} must be a directory") from error
    if create_parent:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
        except FileExistsError as error:
            raise EvidenceError(f"{label} must be a directory") from error
        except OSError as error:
            detail = _safe_os_error_detail(error)
            raise EvidenceError(f"cannot create {label} parent: {detail}") from error
        except (RuntimeError, TypeError, ValueError):
            raise EvidenceError(f"cannot create {label} parent: I/O error") from None
    def inspected_exists(target: Path, role: str) -> bool:
        try:
            return target.exists() or target.is_symlink()
        except OSError as error:
            detail = _safe_os_error_detail(error)
            raise EvidenceError(
                f"cannot inspect {label} {role}: {detail}"
            ) from error
        except (RuntimeError, TypeError, ValueError):
            raise EvidenceError(
                f"cannot inspect {label} {role}: I/O error"
            ) from None

    def inspected_lstat(target: Path, role: str) -> os.stat_result:
        try:
            return target.lstat()
        except OSError as error:
            detail = _safe_os_error_detail(error)
            raise EvidenceError(
                f"cannot inspect {label} {role}: {detail}"
            ) from error
        except (RuntimeError, TypeError, ValueError):
            raise EvidenceError(
                f"cannot inspect {label} {role}: I/O error"
            ) from None

    if inspected_exists(path.parent, "parent"):
        parent_mode = inspected_lstat(path.parent, "parent").st_mode
        if stat.S_ISLNK(parent_mode):
            raise EvidenceError(f"{label} must not be a symlink")
        if not stat.S_ISDIR(parent_mode):
            raise EvidenceError(f"{label} must be a directory")
    if inspected_exists(path, "leaf"):
        metadata = inspected_lstat(path, "leaf")
        if stat.S_ISLNK(metadata.st_mode):
            raise EvidenceError(f"{label} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise EvidenceError(f"{label} must be a regular file")
        if metadata.st_nlink > 1:
            raise EvidenceError(f"{label} must not be hard-linked")


def _write_text_output(path: Path, text: str, *, display_label: str | None = None) -> None:
    label = display_label if display_label is not None else "output path"
    _ensure_text_output_target(path, display_label=label)
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise EvidenceError(f"{label} must not be a symlink") from error
        raise EvidenceError(f"{label} must be a directory") from error

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
                raise EvidenceError(f"{label} temp file must not be a symlink") from error
            detail = _safe_os_error_detail(error)
            raise EvidenceError(
                f"cannot open temporary output for {label}: {detail}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise EvidenceError(f"{label} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise EvidenceError(f"{label} temp file must not be hard-linked")
        try:
            with os.fdopen(fd, "w", encoding="utf-8") as handle:
                fd = -1
                handle.write(text)
                handle.flush()
                os.fsync(handle.fileno())
        except OSError as error:
            detail = _safe_os_error_detail(error)
            raise EvidenceError(
                f"cannot write temporary output for {label}: {detail}"
            ) from error
        except (RuntimeError, TypeError, ValueError):
            raise EvidenceError(
                f"cannot write temporary output for {label}: I/O error"
            ) from None
        try:
            os.replace(tmp_name, path.name, src_dir_fd=parent_fd, dst_dir_fd=parent_fd)
        except OSError as error:
            detail = _safe_os_error_detail(error)
            raise EvidenceError(f"cannot replace {label}: {detail}") from error
        except (RuntimeError, TypeError, ValueError):
            raise EvidenceError(f"cannot replace {label}: I/O error") from None
        tmp_created = False
        try:
            os.fsync(parent_fd)
        except (OSError, RuntimeError, TypeError, ValueError):
            pass
    finally:
        if fd >= 0:
            try:
                os.close(fd)
            except (OSError, RuntimeError, TypeError, ValueError):
                pass
        if tmp_created:
            try:
                os.unlink(tmp_name, dir_fd=parent_fd)
            except (OSError, RuntimeError, TypeError, ValueError):
                pass
        try:
            os.close(parent_fd)
        except (OSError, RuntimeError, TypeError, ValueError):
            pass


def _reject_symlinked_existing_ancestors(
    path: Path,
    *,
    display_label: str | None = None,
) -> None:
    current = Path(path.anchor) if path.is_absolute() else Path(".")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    for part in parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except FileNotFoundError:
            return
        except NotADirectoryError:
            raise
        except OSError as error:
            detail = _safe_os_error_detail(error)
            label = display_label or str(current)
            raise EvidenceError(
                f"cannot inspect {label} ancestors: {detail}"
            ) from error
        except (RuntimeError, TypeError, ValueError):
            label = display_label or str(current)
            raise EvidenceError(
                f"cannot inspect {label} ancestors: I/O error"
            ) from None
        if stat.S_ISLNK(mode):
            if path.is_absolute() and current.parent == Path(path.anchor):
                continue
            label = display_label or str(current)
            raise EvidenceError(f"{label} must not be a symlink")


def _load_json(path: Path, *, display_label: str | None = None) -> Any:
    label = display_label or str(path)
    try:
        raw = _read_regular_file(
            path,
            max_bytes=MAX_SUMMARY_JSON_BYTES,
            display_label=label,
        )
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise EvidenceError(f"{label} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_int=_parse_canonical_json_int,
            parse_float=_parse_canonical_json_float,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{label} is not valid JSON") from error
    except RecursionError as error:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(value)
    return value


def _pipe_chunk_bytes(chunk: Any) -> bytes:
    if type(chunk) is bytes:
        return chunk
    if type(chunk) is bytearray:
        return bytes(chunk)
    if type(chunk) is memoryview:
        try:
            return chunk.cast("B").tobytes()
        except (TypeError, ValueError):
            raise OSError("child output stream returned invalid bytes") from None
    raise OSError("child output stream returned non-byte data")


def _read_limited_pipe(pipe: Any, limit_bytes: int) -> tuple[bytes, bool]:
    chunks: list[bytes] = []
    remaining = limit_bytes
    truncated = False
    while True:
        chunk = _pipe_chunk_bytes(pipe.read(8192))
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
    try:
        process = subprocess.Popen(
            argv,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
    except (OSError, RuntimeError, TypeError, ValueError):
        raise EvidenceError("receipt verifier could not be started") from None
    outputs: dict[str, tuple[bytes, bool]] = {}
    read_failed = False

    def read_stream(name: str, pipe: Any) -> None:
        nonlocal read_failed
        try:
            outputs[name] = _read_limited_pipe(pipe, output_limit_bytes)
        except (OSError, RuntimeError, TypeError, ValueError):
            read_failed = True
        finally:
            try:
                pipe.close()
            except (OSError, RuntimeError, TypeError, ValueError):
                read_failed = True

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
    wait_failed = False
    try:
        returncode = process.wait(timeout=timeout_secs)
    except subprocess.TimeoutExpired:
        timed_out = True
        try:
            process.kill()
        except (OSError, RuntimeError, TypeError, ValueError):
            pass
        try:
            process.wait()
        except (OSError, RuntimeError, TypeError, ValueError):
            pass
        returncode = 124
    except (OSError, RuntimeError, TypeError, ValueError):
        wait_failed = True
        try:
            process.kill()
        except (OSError, RuntimeError, TypeError, ValueError):
            pass
        returncode = 124
    try:
        stdout_thread.join(timeout=1.0)
        stderr_thread.join(timeout=1.0)
        if stdout_thread.is_alive() or stderr_thread.is_alive():
            read_failed = True
    except (RuntimeError, TypeError, ValueError):
        read_failed = True
    if read_failed:
        raise EvidenceError("receipt verifier output could not be read") from None
    if wait_failed or isinstance(returncode, bool) or not isinstance(returncode, int):
        raise EvidenceError("receipt verifier did not finish cleanly") from None
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
    if len(pairs) > MAX_JSON_OBJECT_MEMBERS:
        raise EvidenceError(
            f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
        )
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise EvidenceError("duplicate key in JSON object")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise EvidenceError("JSON contains non-finite numeric constant")

def _parse_canonical_json_int(value: str) -> int:
    if JSON_CANONICAL_INT_RE.fullmatch(value) is None:
        raise EvidenceError("JSON contains non-canonical numeric value")
    return int(value, 10)


def _parse_canonical_json_float(value: str) -> float:
    if CLI_CANONICAL_NUMBER_RE.fullmatch(value) is None:
        raise EvidenceError("JSON contains non-canonical numeric value")
    parsed = float(value)
    if parsed == float("inf") or parsed == float("-inf"):
        raise EvidenceError("JSON contains non-finite numeric constant")
    if parsed == 0.0 and value.startswith("-"):
        raise EvidenceError("JSON contains non-canonical numeric value")
    return parsed


def _reject_json_surrogates(value: Any, *, _depth: int = 0) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, str):
        value = _plain_text(value, "JSON string")
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise EvidenceError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        if type(value) is not list:
            raise EvidenceError("JSON array must be a plain array")
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise EvidenceError(
                f"JSON array must contain at most {MAX_JSON_LIST_ITEMS} items"
            )
        for item in value:
            _reject_json_surrogates(item, _depth=_depth + 1)
    elif isinstance(value, dict):
        if type(value) is not dict:
            raise EvidenceError("JSON object must be a plain object")
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise EvidenceError(
                f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
            )
        for key, item in value.items():
            _reject_json_surrogates(key, _depth=_depth + 1)
            _reject_json_surrogates(item, _depth=_depth + 1)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise EvidenceError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> set[str]:
    if type(value) is not dict:
        raise EvidenceError(f"{label} contains unknown keys")
    present: set[str] = set()
    for key in value:
        if not isinstance(key, str):
            raise EvidenceError(f"{label} contains unknown keys")
        present.add(_plain_text(key, f"{label} field"))
    if present - allowed:
        raise EvidenceError(f"{label} contains unknown keys")
    return present


def _is_secret_looking_key(value: Any) -> bool:
    markers = (
        "authorization",
        "bearer",
        "token",
        "secret",
        "private_key",
        "private-key",
        "private key",
        "private.key",
        "privatekey",
        "password",
        "passphrase",
        "api_key",
        "api-key",
        "api key",
        "api.key",
        "apikey",
        "access_key",
        "access-key",
        "access key",
        "access.key",
        "accesskey",
        "session_key",
        "session-key",
        "session key",
        "session.key",
        "sessionkey",
        "client_secret",
        "client-secret",
        "client secret",
        "client.secret",
        "clientsecret",
        "cookie",
        "set-cookie",
        "set cookie",
        "set.cookie",
        "setcookie",
        "x-iroha-signature",
        "x_iroha_signature",
        "x iroha signature",
        "x.iroha.signature",
        "xirohasignature",
    )
    return any(
        _contains_secret_marker(candidate, markers)
        for candidate in _secret_scan_values(str(value))
    )


def _is_control_bearing_key(value: Any) -> bool:
    return _contains_control_character(str(value))


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise EvidenceError(f"{label} must not contain secret-looking material")


def _reject_non_ascii_context(value: str, label: str) -> None:
    if any(ord(ch) > 0x7E for ch in value):
        raise EvidenceError(f"{label} must use printable ASCII")


def _reject_overlong_trust_policy(value: str, label: str) -> None:
    if len(value) > MAX_TRUST_POLICY_CHARS:
        raise EvidenceError(
            f"{label} must be no longer than {MAX_TRUST_POLICY_CHARS} characters"
        )


def _reject_overlong_trust_source_text(value: str, label: str) -> None:
    if len(value) > MAX_TRUST_SOURCE_TEXT_CHARS:
        raise EvidenceError(
            f"{label} must be no longer than {MAX_TRUST_SOURCE_TEXT_CHARS} characters"
        )


def _require_list(value: Any, label: str) -> list[Any]:
    if type(value) is not list:
        raise EvidenceError(f"{label} must be a JSON array")
    if len(value) > MAX_JSON_LIST_ITEMS:
        raise EvidenceError(f"{label} must contain at most {MAX_JSON_LIST_ITEMS} items")
    return value


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    field_label = f"{label}.{key}"
    if not isinstance(raw, str):
        raise EvidenceError(f"{field_label} must be a non-empty string")
    raw = _plain_text(raw, field_label)
    if not raw.strip():
        raise EvidenceError(f"{field_label} must be a non-empty string")
    if len(raw) > MAX_CLEAN_STRING_CHARS:
        raise EvidenceError(f"{field_label} must be no longer than {MAX_CLEAN_STRING_CHARS} characters")
    if _contains_control_character(raw):
        raise EvidenceError(f"{field_label} must not contain control characters")
    if raw != raw.strip():
        raise EvidenceError(f"{field_label} must not have surrounding whitespace")
    return raw


def _required_context_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_positive_int_field(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if type(raw) is not int or raw <= 0:
        raise EvidenceError(f"{label}.{key} must be a positive integer")
    return raw


def _required_profile_id(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    if len(raw) > MAX_PROFILE_ID_CHARS:
        raise EvidenceError(
            f"{label}.{key} must be no longer than {MAX_PROFILE_ID_CHARS} characters"
        )
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
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    if MESSAGE_TYPE_RE.fullmatch(raw) is None:
        raise EvidenceError(f"{label}.{key} must be lowercase ISO family id")
    return raw


def _nullable_rail_message_id(value: dict[str, Any], key: str, label: str) -> str | None:
    if key not in value:
        raise EvidenceError(f"{label}.{key} must be recorded")
    raw = value[key]
    if raw is None:
        return None
    if not isinstance(raw, str):
        raise EvidenceError(f"{label}.{key} must be null or a non-empty string")
    raw = _plain_text(raw, f"{label}.{key}")
    if not raw.strip():
        raise EvidenceError(f"{label}.{key} must be null or a non-empty string")
    if _contains_control_character(raw):
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
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_stage_name(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _reject_forbidden_receipt_metadata(
    receipt_entry: dict[str, Any],
    forbidden_keys: set[str],
    entry_label: str,
    receipt_kind: str,
) -> None:
    present_forbidden: list[str] = []
    for raw_key in receipt_entry:
        if not isinstance(raw_key, str):
            raise EvidenceError(f"{entry_label} contains forbidden metadata")
        key = _plain_text(raw_key, f"{entry_label} field")
        if key in forbidden_keys:
            present_forbidden.append(key)
    for key in sorted(present_forbidden):
        raise EvidenceError(f"{entry_label}.{key} is not valid for {receipt_kind}")


def _valid_nonzero_sha256(value: Any) -> bool:
    return isinstance(value, str) and _is_lower_sha256(value) and any(
        ch != "0" for ch in value
    )


def _reject_receipt_digest_role_reuse(
    receipt_entry: dict[str, Any],
    entry_label: str,
    *,
    receipt_kind: str,
) -> None:
    roles = ["receipt_sha256", "response_body_sha256"]
    if receipt_kind == "iso-audit-notary":
        roles.extend(("anchor_sha256", "index_sha256"))
    elif receipt_kind == "iso-rail-gateway":
        roles.append("payload_sha256")
    else:
        return

    seen: dict[str, str] = {}
    for role in roles:
        digest = receipt_entry.get(role)
        if not _valid_nonzero_sha256(digest):
            continue
        if digest in seen:
            raise EvidenceError(
                f"{entry_label}.{role} must not reuse {entry_label}.{seen[digest]}"
            )
        seen[digest] = role


def _verify_receipt_entry_metadata(
    receipt_entry: dict[str, Any],
    entry_label: str,
    *,
    receipt_kind: str,
    allow_legacy_colr007: bool,
    allow_default_profile: bool,
    require_source_files: bool,
) -> None:
    if receipt_kind == "iso-audit-notary":
        _reject_forbidden_receipt_metadata(
            receipt_entry,
            RAIL_RECEIPT_METADATA_KEYS,
            entry_label,
            receipt_kind,
        )
        _required_sha256(receipt_entry, "anchor_sha256", entry_label)
        index_sha256 = _required_sha256(receipt_entry, "index_sha256", entry_label)
        anchor_path = _validate_notary_anchor_path(
            _required_string(receipt_entry, "anchor_path", entry_label),
            f"{entry_label}.anchor_path",
            index_sha256,
        )
        if _receipt_path_is_repository_fixture(anchor_path):
            raise EvidenceError(
                f"{entry_label}.anchor_path must not point to checked-in ISO fixture artifacts"
            )
        store_dir = _validate_notary_store_dir(
            receipt_entry,
            entry_label,
            require_source_files=require_source_files,
        )
        if store_dir is not None and _receipt_path_is_repository_fixture(store_dir):
            raise EvidenceError(
                f"{entry_label}.store_dir must not point to checked-in ISO fixture artifacts"
            )
        index_path = _validate_notary_index_path(
            receipt_entry,
            entry_label,
            anchor_path,
            require_source_files=require_source_files,
        )
        if index_path is not None and _receipt_path_is_repository_fixture(index_path):
            raise EvidenceError(
                f"{entry_label}.index_path must not point to checked-in ISO fixture artifacts"
            )
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
            raise EvidenceError(f"{entry_label}.message_type is unsupported")
        if message_type in LEGACY_RAIL_MESSAGE_TYPES and not allow_legacy_colr007:
            raise EvidenceError(
                f"{entry_label}.message_type uses legacy rail message type"
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
        source_path = _validate_xml_path(
            _required_string(receipt_entry, "source_path", entry_label),
            f"{entry_label}.source_path",
        )
        if _receipt_path_is_repository_fixture(source_path):
            raise EvidenceError(
                f"{entry_label}.source_path must not point to checked-in ISO XML fixtures"
            )
    else:  # pragma: no cover - supported kinds are checked before this helper.
        raise EvidenceError(f"{entry_label}.receipt_kind is unsupported")


def _receipt_entry_content_metadata(receipt_entry: dict[str, Any]) -> tuple[tuple[str, Any], ...]:
    receipt_kind = receipt_entry["receipt_kind"]
    generic_keys = (
        "ok",
        "status_code",
        "response_body_sha256",
        "endpoint_requires_insecure_http",
    )
    if receipt_kind == "iso-audit-notary":
        keys = (
            "anchor_path",
            "store_dir",
            "index_path",
            "anchor_sha256",
            "index_sha256",
            "record_count",
        )
    elif receipt_kind == "iso-rail-gateway":
        keys = (
            "message_type",
            "payload_sha256",
            "profile",
            "rail_message_id",
            "source_path",
        )
    else:  # pragma: no cover - supported kinds are checked before this helper.
        raise EvidenceError("unsupported receipt_kind")
    return tuple((key, receipt_entry.get(key)) for key in (*generic_keys, *keys))


def _receipt_summary_entry_order_key(entry: dict[str, Any]) -> tuple[str, str, str]:
    return (entry["receipt_kind"], entry["path"], entry["receipt_sha256"])


def _required_cli_string(value: str | None, label: str) -> str:
    if value is None:
        raise EvidenceError(f"provide {label}")
    if not isinstance(value, str):
        raise EvidenceError(f"{label} must be a string")
    value = _plain_text(value, label)
    if not value.strip():
        raise EvidenceError(f"provide {label}")
    if any(
        ord(ch) < 0x20 or ord(ch) == 0x7F or unicodedata.category(ch) == "Cf"
        for ch in value
    ):
        raise EvidenceError(f"{label} must not contain control characters")
    if value != value.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    _reject_non_ascii_context(value, label)
    _reject_secret_looking_identifier(value, label)
    return value


def _optional_cli_profile_id(value: str | None, label: str) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise EvidenceError(f"{label} must be a string")
    value = _plain_text(value, label)
    if not value.strip():
        raise EvidenceError(f"{label} requires a profile id value")
    if any(
        ord(ch) < 0x20 or ord(ch) == 0x7F or unicodedata.category(ch) == "Cf"
        for ch in value
    ):
        raise EvidenceError(f"{label} must not contain control characters")
    if value != value.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    _reject_non_ascii_context(value, label)
    if len(value) > MAX_PROFILE_ID_CHARS:
        raise EvidenceError(
            f"{label} must be no longer than {MAX_PROFILE_ID_CHARS} characters"
        )
    if PROFILE_ID_RE.fullmatch(value) is None:
        raise EvidenceError(f"{label} must be a canonical lowercase profile id")
    _reject_secret_looking_identifier(value, label)
    return value


def _required_positive_cli_int(value: int | None, label: str) -> int:
    if value is None:
        raise EvidenceError(f"provide {label}")
    if type(value) is not int or value <= 0:
        raise EvidenceError(f"{label} must be a positive integer")
    return value


def _required_positive_finite_cli_number(value: Any, label: str) -> float:
    if type(value) not in (int, float):
        raise EvidenceError(f"{label} must be a positive finite number")
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise EvidenceError(f"{label} must be a positive finite number")
    return parsed


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(_path_resolve(path, f"{label}[{offset}]"))
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


def _required_cli_bool(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise EvidenceError(f"{label} must be a boolean")
    return value


def _optional_cli_path(value: Any, label: str) -> Path | None:
    if value is None:
        return None
    if isinstance(value, bytes):
        raise EvidenceError(f"{label} must be a path")
    if isinstance(value, str):
        return Path(_plain_text(value, label))
    if isinstance(value, Path):
        return Path(value)
    raise EvidenceError(f"{label} must be a path")


def _required_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if type(raw) is not int or raw < 0:
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
    _reject_all_zero_sha256(raw, f"{label}.{key}")
    return raw


def _reject_all_zero_sha256(value: str, label: str) -> None:
    if all(ch == "0" for ch in value):
        raise EvidenceError(f"{label} must not be all zero")


def _required_nonzero_sha256(value: dict[str, Any], key: str, label: str) -> str:
    digest = _required_sha256(value, key, label)
    _reject_all_zero_sha256(digest, f"{label}.{key}")
    return digest


def _required_sha256_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _require_list(value.get(key), f"{label}.{key}")
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, item in enumerate(items):
        if isinstance(item, str):
            _reject_secret_looking_identifier(item, f"{label}.{key}[{offset}]")
        if not _is_lower_sha256(item):
            raise EvidenceError(f"{label}.{key}[{offset}] must be a canonical SHA-256")
        _reject_all_zero_sha256(item, f"{label}.{key}[{offset}]")
        if item in seen:
            raise EvidenceError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[item]}]"
            )
        seen[item] = offset
        result.append(item)
    if result != sorted(result):
        raise EvidenceError(f"{label}.{key} must be sorted by sha256")
    return result


def _required_clean_string_list(
    value: dict[str, Any],
    key: str,
    label: str,
    *,
    max_chars: int | None = MAX_CLEAN_STRING_CHARS,
) -> list[str]:
    items = _require_list(value.get(key), f"{label}.{key}")
    result: list[str] = []
    for offset, item in enumerate(items):
        item_label = f"{label}.{key}[{offset}]"
        if not isinstance(item, str):
            raise EvidenceError(f"{item_label} must be a non-empty string")
        item = _plain_text(item, item_label)
        if not item.strip():
            raise EvidenceError(f"{item_label} must be a non-empty string")
        if max_chars is not None and len(item) > max_chars:
            raise EvidenceError(
                f"{item_label} must be no longer than {max_chars} characters"
            )
        if _contains_control_character(item):
            raise EvidenceError(f"{item_label} must not contain control characters")
        if item != item.strip():
            raise EvidenceError(f"{item_label} must not have surrounding whitespace")
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
    if result != sorted(result):
        raise EvidenceError(f"{label}.{key} must be sorted in canonical order")
    return result


def _required_canonical_base64_list(
    value: dict[str, Any],
    key: str,
    label: str,
    *,
    der_kind: str,
) -> list[str]:
    items = _required_clean_string_list(value, key, label, max_chars=None)
    if len(items) > MAX_TRUST_DER_BLOBS:
        raise EvidenceError(
            f"{label}.{key} must not contain more than {MAX_TRUST_DER_BLOBS} entries"
        )
    result: list[str] = []
    seen: dict[str, int] = {}
    order_keys: list[tuple[str, int]] = []
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
        _require_der_kind(decoded, f"{label}.{key}[{offset}]", der_kind)
        canonical = base64.b64encode(decoded).decode("ascii")
        if canonical != item:
            raise EvidenceError(f"{label}.{key}[{offset}] must be canonical padded base64")
        if canonical in seen:
            raise EvidenceError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[canonical]}]"
            )
        seen[canonical] = offset
        order_keys.append((sha256_hex(decoded), len(decoded)))
        result.append(canonical)
    if order_keys != sorted(order_keys):
        raise EvidenceError(
            f"{label}.{key} must be sorted by sha256 and byte_len"
        )
    return result


def _read_der_element(data: bytes, offset: int, label: str) -> DerElement:
    if offset < 0 or offset >= len(data) or len(data) - offset < 2:
        raise EvidenceError(f"{label} has truncated DER length")
    tag = data[offset]
    length_byte = data[offset + 1]
    if length_byte < 0x80:
        length = length_byte
        header_len = 2
    else:
        length_octets = length_byte & 0x7F
        if length_octets == 0 or length_octets > 4:
            raise EvidenceError(f"{label} has invalid DER length")
        if len(data) - offset < 2 + length_octets:
            raise EvidenceError(f"{label} has truncated DER length")
        length_bytes = data[offset + 2 : offset + 2 + length_octets]
        if length_bytes[0] == 0:
            raise EvidenceError(f"{label} has non-minimal DER length")
        length = int.from_bytes(length_bytes, "big")
        if length < 0x80:
            raise EvidenceError(f"{label} has non-minimal DER length")
        header_len = 2 + length_octets
    end = offset + header_len + length
    if end > len(data):
        raise EvidenceError(f"{label} DER length does not consume the whole value")
    return DerElement(
        tag=tag,
        header_len=header_len,
        length=length,
        start=offset,
        value_start=offset + header_len,
        end=end,
        value=data[offset + header_len : end],
    )


def _der_children(element: DerElement, label: str) -> list[DerElement]:
    children: list[DerElement] = []
    offset = 0
    while offset < len(element.value):
        child = _read_der_element(element.value, offset, label)
        children.append(child)
        offset = child.end
    return children


def _root_der_children(value: bytes, label: str) -> list[DerElement]:
    root = _read_der_element(value, 0, label)
    if root.tag != 0x30:
        raise EvidenceError(f"{label} must be a DER SEQUENCE")
    if root.end != len(value):
        raise EvidenceError(f"{label} DER length does not consume the whole value")
    return _der_children(root, label)


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


def _require_der_kind(value: bytes, label: str, kind: str) -> None:
    if kind == TRUST_DER_KIND_CRL:
        if not _looks_like_x509_crl(value, label):
            raise EvidenceError(f"{label} must look like an X.509 CRL")
        return
    if kind == TRUST_DER_KIND_OCSP:
        if not _looks_like_ocsp_response(value, label):
            raise EvidenceError(f"{label} must look like an OCSPResponse")
        return
    raise EvidenceError(f"{label} has unsupported DER kind")


def _looks_like_algorithm_identifier(element: DerElement, label: str) -> bool:
    if element.tag != 0x30:
        return False
    children = _der_children(element, label)
    return bool(children) and children[0].tag == 0x06


def _looks_like_x509_crl(value: bytes, label: str) -> bool:
    children = _root_der_children(value, label)
    if len(children) != 3 or children[0].tag != 0x30 or children[2].tag != 0x03:
        return False
    if not _looks_like_algorithm_identifier(children[1], label):
        return False
    tbs_children = _der_children(children[0], label)
    cursor = 1 if tbs_children and tbs_children[0].tag == 0x02 else 0
    if len(tbs_children) < cursor + 3:
        return False
    this_update = tbs_children[cursor + 2]
    return (
        _looks_like_algorithm_identifier(tbs_children[cursor], label)
        and tbs_children[cursor + 1].tag == 0x30
        and this_update.tag in (0x17, 0x18)
    )


def _looks_like_ocsp_response(value: bytes, label: str) -> bool:
    children = _root_der_children(value, label)
    if not children or children[0].tag != 0x0A:
        return False
    if len(children) == 1:
        return True
    if len(children) != 2 or children[1].tag != 0xA0:
        return False
    response_bytes_children = _der_children(children[1], label)
    if len(response_bytes_children) != 1 or response_bytes_children[0].tag != 0x30:
        return False
    wrapped = _der_children(response_bytes_children[0], label)
    return (
        len(wrapped) == 2
        and wrapped[0].tag == 0x06
        and wrapped[0].value == OID_OCSP_BASIC_RESPONSE_DER
        and wrapped[1].tag == 0x04
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
    order_keys: list[tuple[str, int]] = []
    for offset, raw_entry in enumerate(items):
        entry_label = f"{label}.{key}[{offset}]"
        entry = _require_object(raw_entry, entry_label)
        _reject_unknown_keys(entry, TRUST_DER_SUMMARY_KEYS, entry_label)
        if "label" in entry:
            raw_label = entry.get("label")
            if not isinstance(raw_label, str):
                raise EvidenceError(
                    f"{entry_label}.label must be a non-empty string when provided"
                )
            raw_label = _plain_text(raw_label, f"{entry_label}.label")
            if not raw_label.strip():
                raise EvidenceError(
                    f"{entry_label}.label must be a non-empty string when provided"
                )
            if raw_label != raw_label.strip():
                raise EvidenceError(f"{entry_label}.label must not have surrounding whitespace")
            if _contains_control_character(raw_label):
                raise EvidenceError(f"{entry_label}.label must not contain control characters")
            if len(raw_label) > 128:
                raise EvidenceError(f"{entry_label}.label must be no longer than 128 characters")
            _reject_non_ascii_context(raw_label, f"{entry_label}.label")
            _reject_secret_looking_identifier(raw_label, f"{entry_label}.label")
            if raw_label in seen_labels:
                raise EvidenceError(
                    f"{entry_label}.label duplicates {label}.{key}[{seen_labels[raw_label]}].label"
                )
            seen_labels[raw_label] = offset
        digest = entry.get("sha256")
        if not _is_lower_sha256(digest):
            raise EvidenceError(f"{entry_label}.sha256 must be a canonical SHA-256")
        _reject_all_zero_sha256(digest, f"{entry_label}.sha256")
        if digest in result:
            raise EvidenceError(f"{entry_label}.sha256 duplicates DER SHA-256")
        byte_len = _required_nonnegative_int(entry, "byte_len", entry_label)
        if byte_len == 0 or byte_len > MAX_TRUST_DER_BYTES:
            raise EvidenceError(
                f"{entry_label}.byte_len must be positive and no more than "
                f"{MAX_TRUST_DER_BYTES}"
            )
        result[digest] = byte_len
        order_keys.append((digest, byte_len))
    if order_keys != sorted(order_keys):
        raise EvidenceError(f"{label}.{key} must be sorted by sha256 and byte_len")
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
            f"{summary_label} contains DER material missing from {pins_label}"
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
            f"{override_label} contains DER material not recorded in {summary_label}"
        )
    missing = sorted(set(entries) - set(override_entries))
    if missing:
        raise EvidenceError(
            f"{summary_label} contains DER material missing from {override_label}"
        )
    for digest, byte_len in override_entries.items():
        if entries[digest] != byte_len:
            raise EvidenceError(
                f"{summary_label} byte_len does not match {override_label} "
                "for DER material"
            )


def _compact_der_entries(entries: dict[str, int]) -> list[dict[str, int | str]]:
    return [
        {
            "sha256": digest,
            "byte_len": entries[digest],
        }
        for digest in sorted(entries)
    ]


def _trust_bundle_summary_order_key(bundle: dict[str, Any]) -> tuple[str, str, str]:
    return (
        str(bundle["profile_id"]),
        str(bundle["path"]),
        str(bundle["bundle_sha256"]),
    )


def _reject_sha256_overlap(first: list[str], second: list[str], label: str) -> None:
    overlap = sorted(set(first) & set(second))
    if overlap:
        raise EvidenceError(f"{label} contains overlapping SHA-256 pins")


def _validate_receipt_path(raw: str, label: str) -> str:
    raw = _plain_text(raw, label)
    _reject_path_smuggling(raw, label)
    if _receipt_path_is_repository_fixture(raw):
        raise EvidenceError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )
    if not raw.endswith(RECEIPT_PATH_SUFFIX):
        raise EvidenceError(f"{label} must point to a {RECEIPT_PATH_SUFFIX} file")
    return raw


def _validate_config_path(raw: str, label: str) -> str:
    raw = _plain_text(raw, label)
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".json"):
        raise EvidenceError(f"{label} must point to a .json file")
    if _canary_config_path_is_repository_template(raw):
        raise EvidenceError(
            f"{label} must not point to checked-in operator canary templates"
        )
    return raw


def _validate_trust_bundle_path(raw: str, label: str) -> str:
    raw = _plain_text(raw, label)
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".json"):
        raise EvidenceError(f"{label} must point to a .json file")
    if _trust_bundle_path_is_repository_template(raw):
        raise EvidenceError(
            f"{label} must not point to checked-in trust-bundle templates"
        )
    return raw


def _validate_xml_path(raw: str, label: str) -> str:
    raw = _plain_text(raw, label)
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".xml"):
        raise EvidenceError(f"{label} must point to a .xml file")
    return raw


def _validate_notary_anchor_path(raw: str, label: str, index_sha256: str) -> str:
    raw = _plain_text(raw, label)
    _reject_path_smuggling(raw, label)
    parts = raw.split("/")
    leaf = parts[-1] if parts else ""
    if leaf == LATEST_ANCHOR_FILE:
        return raw
    expected_leaf = f"{index_sha256}.notary.json"
    if len(parts) >= 2 and parts[-2] == ANCHOR_DIR and leaf == expected_leaf:
        return raw
    raise EvidenceError(
        f"{label} must be {LATEST_ANCHOR_FILE} or {ANCHOR_DIR}/<index_sha256>.notary.json"
    )


def _validate_notary_store_dir(
    receipt_entry: dict[str, Any],
    entry_label: str,
    *,
    require_source_files: bool,
) -> str | None:
    if "store_dir" not in receipt_entry or receipt_entry["store_dir"] is None:
        if require_source_files:
            raise EvidenceError(f"{entry_label}.store_dir must be recorded")
        return None
    raw = receipt_entry["store_dir"]
    if not isinstance(raw, str):
        raise EvidenceError(f"{entry_label}.store_dir must be a non-empty path")
    return _validate_artifact_path(raw, f"{entry_label}.store_dir")


def _expected_notary_index_path(anchor_path: str) -> str:
    parts = anchor_path.split("/")
    if parts[-1] == LATEST_ANCHOR_FILE:
        export_parts = parts[:-1]
    else:
        export_parts = parts[:-2]
    return "/".join([*export_parts, INDEX_FILE]) if export_parts else INDEX_FILE


def _validate_notary_index_path(
    receipt_entry: dict[str, Any],
    entry_label: str,
    anchor_path: str,
    *,
    require_source_files: bool,
) -> str | None:
    if "index_path" not in receipt_entry or receipt_entry["index_path"] is None:
        if require_source_files:
            raise EvidenceError(f"{entry_label}.index_path must be recorded")
        return None
    raw = receipt_entry["index_path"]
    if not isinstance(raw, str):
        raise EvidenceError(f"{entry_label}.index_path must be a non-empty path")
    index_path = _validate_artifact_path(raw, f"{entry_label}.index_path")
    if _receipt_path_is_repository_fixture(index_path):
        raise EvidenceError(
            f"{entry_label}.index_path must not point to checked-in ISO fixture artifacts"
        )
    if index_path != _expected_notary_index_path(anchor_path):
        raise EvidenceError(
            f"{entry_label}.index_path must be the {INDEX_FILE} peer of anchor_path"
        )
    return index_path


def _validate_artifact_path(raw: str, label: str) -> str:
    raw = _plain_text(raw, label)
    if not raw.strip():
        raise EvidenceError(f"{label} must be a non-empty path")
    _reject_path_smuggling(raw, label)
    return raw


def _path_contains_component_sequence(raw: str, components: tuple[str, ...]) -> bool:
    parts = [part.casefold() for part in raw.split("/") if part]
    target = [part.casefold() for part in components]
    if len(parts) < len(target):
        return False
    last_start = len(parts) - len(target)
    return any(
        parts[offset : offset + len(target)] == target
        for offset in range(last_start + 1)
    )


def _canary_config_path_is_repository_template(raw: str) -> bool:
    return _path_contains_component_sequence(raw, REPOSITORY_CANARY_RUNBOOK_PARTS)


def _trust_bundle_path_is_repository_template(raw: str) -> bool:
    return _path_contains_component_sequence(raw, REPOSITORY_TRUST_BUNDLE_PARTS)


def _receipt_path_is_repository_fixture(raw: str) -> bool:
    return _path_contains_component_sequence(raw, REPOSITORY_XML_FIXTURE_PARTS)


def _reject_path_smuggling(raw: str, label: str) -> None:
    raw = _plain_text(raw, label)
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise EvidenceError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
        raise EvidenceError(f"{label} must not contain control characters")
    if any(ord(ch) > 0x7E for ch in raw):
        raise EvidenceError(f"{label} must use printable ASCII")
    if raw != raw.strip():
        raise EvidenceError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise EvidenceError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise EvidenceError(f"{label} must not start with a dash")
    if ";" in raw:
        raise EvidenceError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise EvidenceError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise EvidenceError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
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
    _reject_all_zero_sha256(expected, f"{label}.{SUMMARY_DIGEST_FIELD}")
    body = dict(summary)
    body.pop(SUMMARY_DIGEST_FIELD)
    actual = sha256_hex(_canonical_json_bytes(body))
    if actual != expected:
        raise EvidenceError(f"{label} {SUMMARY_DIGEST_FIELD} mismatch")
    return expected


def _verify_receipt_verifier_summary(
    receipt_obj: dict[str, Any],
    label: str,
    args: argparse.Namespace,
    *,
    allow_partial_receipt_kinds: bool = False,
) -> dict[str, Any]:
    digest = _require_summary_digest(receipt_obj, label)
    _reject_unknown_keys(receipt_obj, RECEIPT_SUMMARY_KEYS, label)
    _check_no_secret_material(receipt_obj, label)
    version = receipt_obj.get("version")
    if type(version) is not int or version != RECEIPT_SUMMARY_VERSION:
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
    receipt_kind_clean: list[str] = []
    seen_receipt_kinds: dict[str, int] = {}
    for offset, item in enumerate(receipt_kind):
        item_label = f"{label}.receipt_kind[{offset}]"
        if not isinstance(item, str):
            raise EvidenceError(f"{label}.receipt_kind must contain strings")
        item = _plain_text(item, item_label)
        if not item.strip():
            raise EvidenceError(f"{label}.receipt_kind must contain strings")
        if _contains_control_character(item):
            raise EvidenceError(
                f"{item_label} must not contain control characters"
            )
        if item != item.strip():
            raise EvidenceError(
                f"{item_label} must not have surrounding whitespace"
            )
        _reject_non_ascii_context(item, item_label)
        _reject_secret_looking_identifier(item, item_label)
        if item in seen_receipt_kinds:
            raise EvidenceError(
                f"{item_label} duplicates "
                f"{label}.receipt_kind[{seen_receipt_kinds[item]}]"
            )
        seen_receipt_kinds[item] = offset
        receipt_kind_clean.append(item)
    receipt_kind_set = set(receipt_kind_clean)
    if allow_partial_receipt_kinds:
        if not (receipt_kind_set & REQUIRED_RECEIPT_KINDS):
            raise EvidenceError(f"{label} has no rail/notary receipt kinds")
    else:
        missing = sorted(REQUIRED_RECEIPT_KINDS - receipt_kind_set)
        if missing:
            raise EvidenceError(f"{label} is missing receipt kinds: {', '.join(missing)}")
    unsupported = sorted(receipt_kind_set - REQUIRED_RECEIPT_KINDS)
    if unsupported:
        raise EvidenceError(f"{label} contains unsupported receipt kinds")
    if receipt_kind_clean != sorted(receipt_kind_set):
        raise EvidenceError(f"{label}.receipt_kind must be sorted in canonical order")

    receipt_entries_raw = _require_list(receipt_obj.get("receipts"), f"{label}.receipts")
    if len(receipt_entries_raw) != verified_receipts:
        raise EvidenceError(f"{label}.receipts length does not match verified_receipts")
    receipt_entries: list[dict[str, Any]] = []
    receipt_entry_kinds: set[str] = set()
    seen_receipt_paths: dict[str, int] = {}
    seen_receipt_digests: dict[str, int] = {}
    receipt_order_keys: list[tuple[str, str, str]] = []
    seen_source_material_signatures: dict[tuple[str, tuple[tuple[str, str], ...]], int] = {}
    seen_source_material_fields: dict[tuple[str, str], dict[str, int]] = {}
    has_failed_receipt = False
    has_insecure_receipt_endpoint = False
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
            raise EvidenceError(f"{entry_label}.receipt_kind is unsupported")
        receipt_sha256 = receipt_entry.get("receipt_sha256")
        if not _is_lower_sha256(receipt_sha256):
            raise EvidenceError(f"{entry_label}.receipt_sha256 must be a canonical SHA-256")
        _reject_all_zero_sha256(receipt_sha256, f"{entry_label}.receipt_sha256")
        if receipt_sha256 in seen_receipt_digests:
            raise EvidenceError(
                f"{entry_label}.receipt_sha256 duplicates "
                f"{label}.receipts[{seen_receipt_digests[receipt_sha256]}].receipt_sha256"
            )
        seen_receipt_digests[receipt_sha256] = offset
        receipt_order_keys.append(
            _receipt_summary_entry_order_key(
                {
                    "receipt_kind": entry_kind,
                    "path": receipt_path,
                    "receipt_sha256": receipt_sha256,
                }
            )
        )
        ok = receipt_entry.get("ok")
        if not isinstance(ok, bool):
            raise EvidenceError(f"{entry_label}.ok must be a boolean")
        status_code = receipt_entry.get("status_code")
        if "status_code" not in receipt_entry or (
            status_code is not None
            and (
                type(status_code) is not int
                or status_code < 100
                or status_code > 599
            )
        ):
            raise EvidenceError(
                f"{entry_label}.status_code must be an HTTP status integer or null"
            )
        status_success = type(status_code) is int and 200 <= status_code <= 299
        if ok != status_success:
            raise EvidenceError(f"{entry_label}.ok does not match status_code success state")
        if not ok and not allow_failed:
            raise EvidenceError(f"{entry_label} did not succeed")
        has_failed_receipt = has_failed_receipt or not ok
        response_body_sha256 = receipt_entry.get("response_body_sha256")
        if status_code is None:
            if "response_body_sha256" not in receipt_entry or response_body_sha256 is not None:
                raise EvidenceError(
                    f"{entry_label}.response_body_sha256 must be null without HTTP status_code"
                )
        elif (
            "response_body_sha256" not in receipt_entry
            or not _is_lower_sha256(response_body_sha256)
        ):
            raise EvidenceError(
                f"{entry_label}.response_body_sha256 must be a canonical SHA-256"
            )
        else:
            _reject_all_zero_sha256(
                response_body_sha256,
                f"{entry_label}.response_body_sha256",
            )
        endpoint_requires_insecure_http = receipt_entry.get(
            "endpoint_requires_insecure_http"
        )
        if not isinstance(endpoint_requires_insecure_http, bool):
            raise EvidenceError(
                f"{entry_label}.endpoint_requires_insecure_http must be a boolean"
            )
        if endpoint_requires_insecure_http and not allow_insecure_http:
            raise EvidenceError(
                f"{entry_label}.endpoint_requires_insecure_http requires "
                "allow_insecure_http=true"
            )
        has_insecure_receipt_endpoint = (
            has_insecure_receipt_endpoint or endpoint_requires_insecure_http
        )
        _verify_receipt_entry_metadata(
            receipt_entry,
            entry_label,
            receipt_kind=entry_kind,
            allow_legacy_colr007=allow_legacy_colr007,
            allow_default_profile=allow_default_profile,
            require_source_files=require_source_files,
        )
        _reject_receipt_digest_role_reuse(
            receipt_entry,
            entry_label,
            receipt_kind=entry_kind,
        )
        source_material_fields = RECEIPT_SOURCE_MATERIAL_FIELDS_BY_KIND.get(
            entry_kind,
            (),
        )
        source_material_values = tuple(
            (field, receipt_entry.get(field)) for field in source_material_fields
        )
        string_source_material_values = tuple(
            (field, value)
            for field, value in source_material_values
            if isinstance(value, str)
        )
        if string_source_material_values:
            source_material_signature = (entry_kind, string_source_material_values)
            if source_material_signature in seen_source_material_signatures:
                first_offset = seen_source_material_signatures[source_material_signature]
                field = string_source_material_values[0][0]
                raise EvidenceError(
                    f"{entry_label}.{field} duplicates "
                    f"{label}.receipts[{first_offset}].{field}"
                )
            seen_source_material_signatures[source_material_signature] = offset
            for field, value in string_source_material_values:
                seen_for_field = seen_source_material_fields.setdefault(
                    (entry_kind, field),
                    {},
                )
                first_offset = seen_for_field.get(value)
                if first_offset is not None:
                    raise EvidenceError(
                        f"{entry_label}.{field} duplicates "
                        f"{label}.receipts[{first_offset}].{field}"
                    )
                seen_for_field[value] = offset
        receipt_entry_kinds.add(entry_kind)
        receipt_entries.append(dict(receipt_entry))
    if receipt_order_keys != sorted(receipt_order_keys):
        raise EvidenceError(
            f"{label}.receipts must be sorted by receipt_kind, path, and receipt_sha256"
        )
    if receipt_kind_set != receipt_entry_kinds:
        raise EvidenceError(f"{label}.receipt_kind does not match receipts[].receipt_kind")
    if allow_failed and not has_failed_receipt:
        raise EvidenceError(f"{label}.allow_failed requires at least one failed receipt")
    if allow_insecure_http and not has_insecure_receipt_endpoint:
        raise EvidenceError(
            f"{label}.allow_insecure_http requires at least one http:// "
            "or local/private receipt endpoint"
        )

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


def _check_no_secret_material(value: Any, label: str = "$", *, _depth: int = 0) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, dict):
        if type(value) is not dict:
            raise EvidenceError(f"{label} contains non-plain JSON object")
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise EvidenceError(
                f"{label} must contain at most {MAX_JSON_OBJECT_MEMBERS} object members"
            )
        for key, child in value.items():
            if not isinstance(key, str):
                raise EvidenceError(f"{label} contains forbidden non-string field")
            key_text = _plain_text(key, f"{label} field")
            if _is_secret_looking_key(key_text):
                raise EvidenceError(f"{label} contains forbidden secret-looking field")
            if _is_control_bearing_key(key_text):
                raise EvidenceError(f"{label} contains forbidden control-bearing field")
            _check_no_secret_material(child, f"{label}.{key_text}", _depth=_depth + 1)
    elif isinstance(value, list):
        if type(value) is not list:
            raise EvidenceError(f"{label} contains non-plain JSON array")
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise EvidenceError(
                f"{label} must contain at most {MAX_JSON_LIST_ITEMS} items"
            )
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{label}[{offset}]", _depth=_depth + 1)
    elif isinstance(value, str):
        value = _plain_text(value, label)
        if _contains_unsafe_preview_control(value):
            raise EvidenceError(f"{label} contains unsafe control characters")
        _reject_secret_string(value, label)


def _command_has_script(command: list[str], script_name: str) -> bool:
    return len(command) > 1 and Path(command[1]).name == script_name


def _require_command_strings(command: list[Any], label: str) -> list[str]:
    normalized: list[str] = []
    for offset, item in enumerate(command):
        if not isinstance(item, str):
            raise EvidenceError(f"{label}.command must contain strings")
        item = _plain_text(item, f"{label}.command[{offset}]")
        if not item:
            raise EvidenceError(f"{label}.command must contain non-empty strings")
        normalized.append(item)
    return normalized


def _command_has_flag(command: list[str], flag: str) -> bool:
    return any(item == flag or item.startswith(flag + "=") for item in command)


def _command_flag_count(command: list[str], flag: str) -> int:
    prefix = flag + "="
    return sum(1 for item in command if item == flag or item.startswith(prefix))


def _stage_command_mode_summary(stage_name: str, command: list[str]) -> dict[str, Any]:
    return {
        "name": stage_name,
        "rail_uses_message": (
            stage_name == "rail" and _command_has_flag(command, "--message")
        ),
        "rail_submitted_message_count": 0,
        "notary_uses_all": (
            stage_name == "notary" and _command_has_flag(command, "--all")
        ),
        "notary_endpoint_count": _command_flag_count(command, "--endpoint")
        if stage_name == "notary"
        else 0,
        "notary_published_anchor_count": 0,
    }


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
            if any(ord(ch) > 0x7E for ch in value):
                raise EvidenceError(
                    f"{label}.command[{offset}] {flag} must be a positive finite number"
                )
            if CLI_CANONICAL_NUMBER_RE.fullmatch(value) is None:
                raise EvidenceError(
                    f"{label}.command[{offset}] {flag} must be a positive finite number"
                )
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


def _check_executed_command_repository_fixture_paths(
    stage_name: str,
    command: list[str],
    label: str,
) -> None:
    if "--receipt-dir" in EXPECTED_STAGE_FLAGS.get(stage_name, set()):
        for offset, value in _command_flag_values(command, "--receipt-dir", label):
            if _receipt_path_is_repository_fixture(value):
                raise EvidenceError(
                    f"{label}.command[{offset}] must not point to checked-in "
                    "ISO fixture artifacts"
                )
    for flag in sorted(STAGE_ARTIFACT_PATH_FLAGS.get(stage_name, set())):
        if flag == "--receipt-dir":
            continue
        for offset, value in _command_flag_values(command, flag, label):
            if _receipt_path_is_repository_fixture(value):
                raise EvidenceError(
                    f"{label}.command[{offset}] must not point to checked-in "
                    "ISO fixture artifacts"
                )
    for flag in sorted(STAGE_XML_PATH_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            if _receipt_path_is_repository_fixture(value):
                raise EvidenceError(
                    f"{label}.command[{offset}] must not point to checked-in "
                    "ISO XML fixtures"
                )
    for flag in sorted(STAGE_RECEIPT_PATH_FLAGS.get(stage_name, set())):
        for offset, value in _command_flag_values(command, flag, label):
            if _receipt_path_is_repository_fixture(value):
                raise EvidenceError(
                    f"{label}.command[{offset}] must not point to checked-in "
                    "ISO fixture artifacts"
                )


def _check_stage_command_repository_fixture_paths(
    stage_name: str,
    command: list[str],
    label: str,
) -> None:
    if stage_name not in EXPECTED_STAGE_FLAGS:
        raise EvidenceError(f"{label}.name has unsupported canary stage")
    _check_executed_command_repository_fixture_paths(stage_name, command, label)


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


def _url_requires_insecure_http_override(parsed: urllib.parse.ParseResult) -> bool:
    if parsed.scheme == "http":
        return True
    hostname = (parsed.hostname or "").strip().lower()
    if hostname == "localhost" or hostname.endswith(".localhost"):
        return True
    if _host_uses_rebinding_suffix(hostname):
        return True
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        return False
    return (not address.is_global) or _address_embeds_non_global_ipv4(address)


def _command_uses_insecure_http_policy(command: list[str], label: str) -> bool:
    for flag in COMMAND_URL_FLAGS:
        for _offset, url in _command_flag_values(command, flag, label):
            if _url_requires_insecure_http_override(urllib.parse.urlparse(url)):
                return True
    return False


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
            if command[offset + 1].startswith("-"):
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
                if not value or value.startswith("-"):
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

    endpoint_values = _command_flag_values(command, "--endpoint", label)
    endpoint_order = [value for _offset, value in endpoint_values]
    seen_endpoints: dict[str, int] = {}
    for offset, endpoint in endpoint_values:
        previous = seen_endpoints.get(endpoint)
        if previous is not None:
            raise EvidenceError(
                f"{label}.command[{offset}] duplicates --endpoint at "
                f"{label}.command[{previous}]"
            )
        seen_endpoints[endpoint] = offset
    if endpoint_order != sorted(endpoint_order):
        raise EvidenceError(f"{label}.command --endpoint values must be sorted")


def _check_redacted_bearer_files(command: list[str], label: str) -> None:
    for offset, item in enumerate(command):
        if item == "--bearer-token-file":
            if offset + 1 >= len(command):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if command[offset + 1].startswith("-"):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if command[offset + 1] != "<runtime-token-file>":
                raise EvidenceError(f"{label} contains an unredacted bearer-token file path")
            continue
        prefix = "--bearer-token-file="
        if item.startswith(prefix):
            value = item[len(prefix) :]
            if not value or value.startswith("-"):
                raise EvidenceError(f"{label} has --bearer-token-file without a value")
            if value != "<runtime-token-file>":
                raise EvidenceError(f"{label} contains an unredacted bearer-token file path")


def _check_command_policy(
    command: list[str],
    label: str,
    *,
    stage_name: str,
    allow_dry_run: bool,
    allow_insecure_http: bool,
    allow_default_profile: bool,
    allow_failed_receipts: bool,
    allow_legacy_colr007: bool,
) -> None:
    if not command:
        raise EvidenceError(f"{label}.command must not be empty")
    for offset, item in enumerate(command):
        if _contains_control_character(item):
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
    command_requires_insecure_http = _command_uses_insecure_http_policy(command, label)
    command_allows_insecure_http = "--allow-insecure-http" in command
    if stage_name in STAGE_RECEIPT_KINDS:
        if command_requires_insecure_http and not command_allows_insecure_http:
            raise EvidenceError(
                f"{label}.command URL requires --allow-insecure-http"
            )
        if command_allows_insecure_http and not command_requires_insecure_http:
            raise EvidenceError(
                f"{label}.command uses --allow-insecure-http without an http:// "
                "or local/private endpoint"
            )
    if _command_has_flag(command, "--allow-default-profile") and not allow_default_profile:
        raise EvidenceError(f"{label} used --allow-default-profile")
    if _command_has_flag(command, "--allow-failed") and not allow_failed_receipts:
        raise EvidenceError(f"{label} allowed failed receipts")
    if _command_has_flag(command, "--allow-legacy-colr007") and not allow_legacy_colr007:
        raise EvidenceError(f"{label} used --allow-legacy-colr007")


def _check_stage_script(stage_name: str, command: list[str], label: str) -> None:
    expected = EXPECTED_STAGE_SCRIPTS.get(stage_name)
    if expected is None:
        raise EvidenceError(f"{label}.name has unsupported canary stage")
    if len(command) < 2:
        raise EvidenceError(
            f"{label}.command must start with a Python interpreter and {expected}"
        )
    interpreter = command[0]
    _reject_path_smuggling(interpreter, f"{label}.command[0]")
    interpreter_name = Path(interpreter).name.lower()
    if (
        re.fullmatch(
            r"(?:python|pypy)(?:[0-9]+(?:\.[0-9]+)*)?(?:\.exe)?",
            interpreter_name,
        )
        is None
    ):
        raise EvidenceError(f"{label}.command[0] must be a Python interpreter path")
    script = command[1]
    _reject_path_smuggling(script, f"{label}.command[1]")
    if not _command_has_script(command, expected):
        raise EvidenceError(f"{label}.command does not invoke {expected}")


def _check_canary_stage_sequence(stage_names: list[str], label: str) -> None:
    stage_name_set = set(stage_names)
    unsupported = sorted(stage_name_set - REQUIRED_CANARY_STAGES)
    if unsupported:
        raise EvidenceError(f"{label} contains unsupported canary stages")
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
        raise EvidenceError(f"{label}.name has unsupported canary stage")
    local_only = LOCAL_DIAGNOSTIC_STAGE_FLAGS.get(stage_name, set())
    boolean_flags = STAGE_BOOLEAN_FLAGS.get(stage_name, set())
    value_offsets = _command_separate_value_offsets(
        command,
        allowed - boolean_flags,
        label,
    )
    for offset, item in enumerate(command):
        if offset < 2:
            continue
        if offset in value_offsets:
            continue
        if not item.startswith("--"):
            raise EvidenceError(
                f"{label}.command[{offset}] uses unsupported positional argument"
            )
        flag = item.split("=", 1)[0]
        if flag not in allowed:
            if any(ord(ch) > 0x7E for ch in flag):
                raise EvidenceError(f"{label}.command[{offset}] uses unsupported flag")
            if _contains_secret_material(item) or _contains_secret_identifier_material(item):
                raise EvidenceError(
                    f"{label}.command[{offset}] uses unsupported secret-looking flag"
                )
        if flag in local_only:
            raise EvidenceError(
                f"{label}.command[{offset}] uses local diagnostic flag; "
                "production evidence must include persisted source records"
            )
        if flag not in allowed:
            raise EvidenceError(f"{label}.command[{offset}] uses unsupported flag")
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


def _check_stage_receipt_dir_scope(stage: dict[str, Any], name: str, label: str) -> None:
    if name not in STAGE_RECEIPT_KINDS and stage.get("receipt_dir") is not None:
        raise EvidenceError(f"{label}.receipt_dir must be null for {name} stage")


def _verify_receipt_stdout(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    if _required_bool(stage, "stdout_truncated", label):
        raise EvidenceError(f"{label}.stdout_preview is truncated")
    stdout = stage.get("stdout_preview")
    stdout_label = f"{label}.stdout_preview"
    if not isinstance(stdout, str):
        raise EvidenceError(f"{stdout_label} must contain receipt verifier JSON")
    stdout = _plain_text(stdout, stdout_label)
    if not stdout.strip():
        raise EvidenceError(f"{label}.stdout_preview must contain receipt verifier JSON")
    try:
        receipt_summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_int=_parse_canonical_json_int,
            parse_float=_parse_canonical_json_float,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(f"{label}.stdout_preview is not valid receipt verifier JSON") from error
    except RecursionError as error:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(receipt_summary)
    receipt_obj = _require_object(receipt_summary, f"{label}.stdout_preview")
    return _verify_receipt_verifier_summary(
        receipt_obj,
        f"{label}.stdout_preview",
        args,
        allow_partial_receipt_kinds=True,
    )


def _receipt_parent_dir(path: str) -> str:
    return path.rsplit("/", 1)[0] if "/" in path else "."


def _check_verify_receipt_selectors(
    command: list[str],
    label: str,
) -> tuple[list[str], list[str]]:
    receipt_dirs = [
        (
            offset,
            _validate_artifact_path(value, f"{label}.command[{offset}]"),
        )
        for offset, value in _command_flag_values(command, "--receipt-dir", label)
    ]
    receipt_files = [
        (
            offset,
            _validate_receipt_path(value, f"{label}.command[{offset}]"),
        )
        for offset, value in _command_flag_values(command, "--receipt", label)
    ]

    seen_dirs: dict[str, int] = {}
    for offset, receipt_dir in receipt_dirs:
        previous = seen_dirs.get(receipt_dir)
        if previous is not None:
            raise EvidenceError(
                f"{label}.command[{offset}] duplicates --receipt-dir at "
                f"{label}.command[{previous}]"
            )
        seen_dirs[receipt_dir] = offset

    seen_files: dict[str, int] = {}
    selected_dirs = set(seen_dirs)
    for offset, receipt_file in receipt_files:
        previous = seen_files.get(receipt_file)
        if previous is not None:
            raise EvidenceError(
                f"{label}.command[{offset}] duplicates --receipt at "
                f"{label}.command[{previous}]"
            )
        seen_files[receipt_file] = offset
        if _receipt_parent_dir(receipt_file) in selected_dirs:
            raise EvidenceError(
                f"{label}.command[{offset}] --receipt is already covered by "
                "--receipt-dir"
            )

    receipt_dir_order = [receipt_dir for _offset, receipt_dir in receipt_dirs]
    if receipt_dir_order != sorted(receipt_dir_order):
        raise EvidenceError(f"{label}.command --receipt-dir values must be sorted")
    receipt_file_order = [receipt_file for _offset, receipt_file in receipt_files]
    if receipt_file_order != sorted(receipt_file_order):
        raise EvidenceError(f"{label}.command --receipt values must be sorted")

    return (
        [receipt_dir for _offset, receipt_dir in receipt_dirs],
        [receipt_file for _offset, receipt_file in receipt_files],
    )


def _check_receipt_stdout_policy_binding(
    command: list[str],
    receipt_summary: dict[str, Any],
    label: str,
) -> None:
    checks = (
        ("--allow-failed", "allow_failed"),
        ("--allow-insecure-http", "allow_insecure_http"),
        ("--allow-legacy-colr007", "allow_legacy_colr007"),
        ("--allow-default-profile", "allow_default_profile"),
        ("--require-source-files", "require_source_files"),
    )
    for flag, field in checks:
        command_has_flag = _command_has_flag(command, flag)
        if receipt_summary[field] != command_has_flag:
            raise EvidenceError(
                f"{label}.stdout_preview.{field} does not match command {flag}"
            )


def _check_receipt_stdout_selector_binding(
    receipt_dirs: list[str],
    receipt_files: list[str],
    receipt_summary: dict[str, Any],
    label: str,
) -> None:
    selected_dirs = set(receipt_dirs)
    selected_files = set(receipt_files)
    receipt_paths = [receipt["path"] for receipt in receipt_summary["receipts"]]
    for offset, receipt_path in enumerate(receipt_paths):
        if (
            receipt_path not in selected_files
            and _receipt_parent_dir(receipt_path) not in selected_dirs
        ):
            raise EvidenceError(
                f"{label}.stdout_preview.receipts[{offset}].path must be covered "
                "by verify command receipt selectors"
            )
    if selected_files - set(receipt_paths):
        raise EvidenceError(
            f"{label}.stdout_preview.receipts must include every verify command "
            "--receipt file"
        )


def _check_stage_output_not_truncated(stage: dict[str, Any], label: str) -> None:
    if _required_bool(stage, "stdout_truncated", label):
        raise EvidenceError(f"{label}.stdout_preview is truncated")
    if _required_bool(stage, "stderr_truncated", label):
        raise EvidenceError(f"{label}.stderr_preview is truncated")


def _required_stage_preview(stage: dict[str, Any], key: str, label: str) -> str:
    preview = stage.get(key)
    preview_label = f"{label}.{key}"
    if not isinstance(preview, str):
        raise EvidenceError(f"{preview_label} must be a string")
    preview = _plain_text(preview, preview_label)
    if _contains_unsafe_preview_control(preview):
        raise EvidenceError(f"{preview_label} contains unsafe control characters")
    if _contains_secret_material(preview) or _contains_secret_identifier_material(preview):
        raise EvidenceError(f"{preview_label} contains secret-looking material")
    return preview


def _stage_stdout_object(stage: dict[str, Any], label: str) -> dict[str, Any]:
    stdout = _required_stage_preview(stage, "stdout_preview", label)
    if not stdout.strip():
        raise EvidenceError(f"{label}.stdout_preview must contain adapter summary JSON")
    try:
        summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_int=_parse_canonical_json_int,
            parse_float=_parse_canonical_json_float,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError(
            f"{label}.stdout_preview is not valid adapter summary JSON"
        ) from error
    except RecursionError as error:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(summary)
    return _require_object(summary, f"{label}.stdout_preview")


def _stage_stdout_receipts(
    summary: dict[str, Any],
    label: str,
    receipt_dir: str,
) -> list[str]:
    receipt_values = _require_list(
        summary.get("receipts"),
        f"{label}.stdout_preview.receipts",
    )
    receipts: list[str] = []
    seen: set[str] = set()
    for offset, raw in enumerate(receipt_values):
        receipt_label = f"{label}.stdout_preview.receipts[{offset}]"
        if not isinstance(raw, str):
            raise EvidenceError(f"{receipt_label} must be a non-empty receipt path")
        receipt_path = _validate_receipt_path(raw, receipt_label)
        if _receipt_parent_dir(receipt_path) != receipt_dir:
            raise EvidenceError(f"{receipt_label} must be under stage receipt_dir")
        if receipt_path in seen:
            raise EvidenceError(f"{receipt_label} duplicates an earlier receipt path")
        seen.add(receipt_path)
        receipts.append(receipt_path)
    return receipts


def _check_stage_adapter_stdout(
    stage_name: str,
    stage: dict[str, Any],
    label: str,
    receipt_dir: str,
    command: list[str],
) -> dict[str, Any]:
    summary = _stage_stdout_object(stage, label)
    summary_label = f"{label}.stdout_preview"
    _reject_unknown_keys(
        summary,
        RAIL_STAGE_STDOUT_KEYS if stage_name == "rail" else NOTARY_STAGE_STDOUT_KEYS,
        summary_label,
    )
    failures = _required_nonnegative_int(summary, "failures", summary_label)
    if failures != 0:
        raise EvidenceError(f"{summary_label}.failures must be zero")
    receipts = _stage_stdout_receipts(summary, label, receipt_dir)
    if stage_name == "rail":
        submitted_messages = _required_positive_int_field(
            summary,
            "submitted_messages",
            summary_label,
        )
        if len(receipts) != submitted_messages:
            raise EvidenceError(
                f"{summary_label}.receipts must match submitted_messages"
            )
        if _command_has_flag(command, "--message") and submitted_messages != 1:
            raise EvidenceError(
                f"{summary_label}.submitted_messages must be one when command uses --message"
            )
        return {
            "receipts": receipts,
            "rail_submitted_message_count": submitted_messages,
        }
    published_anchors = _required_positive_int_field(
        summary,
        "published_anchors",
        summary_label,
    )
    endpoint_count = _required_positive_int_field(
        summary,
        "endpoint_count",
        summary_label,
    )
    if endpoint_count != _command_flag_count(command, "--endpoint"):
        raise EvidenceError(
            f"{summary_label}.endpoint_count does not match command --endpoint count"
        )
    if len(receipts) != published_anchors * endpoint_count:
        raise EvidenceError(
            f"{summary_label}.receipts must match published_anchors and endpoint_count"
        )
    if not _command_has_flag(command, "--all") and published_anchors != 1:
        raise EvidenceError(
            f"{summary_label}.published_anchors must be one unless command uses --all"
        )
    return {
        "receipts": receipts,
        "notary_published_anchor_count": published_anchors,
    }


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
    if stage.get("reason") is not None:
        raise EvidenceError(f"{label}.reason must be null for successful stage")
    if _required_bool(stage, "timed_out", label):
        raise EvidenceError(f"{label} timed out")
    returncode = stage.get("returncode")
    if isinstance(returncode, bool) or not isinstance(returncode, int):
        raise EvidenceError(f"{label}.returncode must be an integer")
    if returncode != 0:
        raise EvidenceError(f"{label} failed with returncode {returncode}")
    _check_stage_output_not_truncated(stage, label)
    _required_stage_preview(stage, "stdout_preview", label)
    stderr_preview = _required_stage_preview(stage, "stderr_preview", label)
    if stderr_preview.strip():
        raise EvidenceError(f"{label}.stderr_preview must be empty for successful stage")
    command = _require_list(stage.get("command"), f"{label}.command")
    command = _require_command_strings(command, label)
    _check_command_policy(
        command,
        label,
        stage_name=name,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    command_uses_dry_run = _command_has_flag(command, "--dry-run")
    command_uses_insecure_http = _command_uses_insecure_http_policy(command, label)
    _check_stage_script(name, command, label)
    _check_stage_receipt_dir_scope(stage, name, label)
    validated_receipt_dir = None
    if name in {"rail", "notary"}:
        receipt_dir = stage.get("receipt_dir")
        if not isinstance(receipt_dir, str):
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        receipt_dir = _plain_text(receipt_dir, f"{label}.receipt_dir")
        if not receipt_dir.strip():
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        _check_receipt_dir_binding(command, receipt_dir, label)
        validated_receipt_dir = _validate_artifact_path(
            receipt_dir,
            f"{label}.receipt_dir",
        )
    _check_stage_command_flags(name, command, label)
    _check_stage_command_repository_fixture_paths(name, command, label)
    if name == "verify":
        receipt_dirs, receipt_files = _check_verify_receipt_selectors(command, label)
        result = {
            "name": name,
            "_started_at": started_at,
            "_finished_at": finished_at,
            "_dry_run": command_uses_dry_run,
            "_receipt_dirs": receipt_dirs,
            "_receipt_files": receipt_files,
            "_command_modes": _stage_command_mode_summary(name, command),
            "started_at": started_at_raw,
            "finished_at": finished_at_raw,
        }
        if (
            not _command_has_flag(command, "--require-source-files")
            and not args.allow_receipt_source_missing
        ):
            raise EvidenceError(f"{label} did not require receipt source files")
        receipt_summary = _verify_receipt_stdout(stage, label, args)
        _check_receipt_stdout_policy_binding(command, receipt_summary, label)
        result["receipt_summary"] = receipt_summary
        result["_uses_dry_run_policy"] = command_uses_dry_run
        result["_uses_insecure_http_policy"] = (
            command_uses_insecure_http or receipt_summary["allow_insecure_http"]
        )
        result["_uses_failed_receipt_policy"] = receipt_summary["allow_failed"]
        return result
    return {
        "name": name,
        "_started_at": started_at,
        "_finished_at": finished_at,
        "_dry_run": command_uses_dry_run,
        "_uses_dry_run_policy": command_uses_dry_run,
        "_uses_insecure_http_policy": command_uses_insecure_http,
        "_uses_default_profile_policy": _command_has_flag(
            command,
            "--allow-default-profile",
        ),
        "_uses_legacy_colr007_policy": _command_has_flag(
            command,
            "--allow-legacy-colr007",
        ),
        "_requires_insecure_http_receipt_kind": STAGE_RECEIPT_KINDS.get(name)
        if command_uses_insecure_http
        else None,
        "_uses_failed_receipt_policy": False,
        "_command_modes": _stage_command_mode_summary(name, command),
        "_command": command,
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
        "receipt_dir": validated_receipt_dir,
    }


def _planned_stage_summary(
    stage: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    _reject_unknown_keys(stage, CANARY_PLANNED_STAGE_KEYS, label)
    name = _required_stage_name(stage, "name", label)
    dry_run = _required_bool(stage, "dry_run", label)
    if dry_run and not args.allow_dry_run:
        raise EvidenceError(f"{label} planned a dry-run stage")
    command = _require_list(stage.get("command"), f"{label}.command")
    command = _require_command_strings(command, label)
    _check_command_policy(
        command,
        label,
        stage_name=name,
        allow_dry_run=args.allow_dry_run,
        allow_insecure_http=args.allow_insecure_http,
        allow_default_profile=args.allow_default_profile,
        allow_failed_receipts=args.allow_failed_receipts,
        allow_legacy_colr007=args.allow_legacy_colr007,
    )
    command_uses_dry_run = _command_has_flag(command, "--dry-run")
    if dry_run != command_uses_dry_run:
        raise EvidenceError(f"{label}.dry_run does not match command --dry-run")
    _check_stage_script(name, command, label)
    _check_stage_receipt_dir_scope(stage, name, label)
    receipt_dir = None
    if name in {"rail", "notary"}:
        receipt_dir = stage.get("receipt_dir")
        if not isinstance(receipt_dir, str):
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        receipt_dir = _plain_text(receipt_dir, f"{label}.receipt_dir")
        if not receipt_dir.strip():
            raise EvidenceError(f"{label}.receipt_dir must be recorded")
        _check_receipt_dir_binding(command, receipt_dir, label)
    _check_stage_command_flags(name, command, label)
    _check_stage_command_repository_fixture_paths(name, command, label)
    receipt_dirs: list[str] = []
    receipt_files: list[str] = []
    if name == "verify":
        receipt_dirs, receipt_files = _check_verify_receipt_selectors(command, label)
    if (
        name == "verify"
        and not _command_has_flag(command, "--require-source-files")
        and not args.allow_receipt_source_missing
    ):
        raise EvidenceError(f"{label} did not require receipt source files")
    return {
        "name": name,
        "_receipt_dirs": receipt_dirs,
        "_receipt_files": receipt_files,
        "_dry_run": dry_run,
        "receipt_dir": _validate_artifact_path(receipt_dir, f"{label}.receipt_dir")
        if name in {"rail", "notary"}
        else None,
        "_uses_dry_run_policy": dry_run,
        "_uses_insecure_http_policy": _command_uses_insecure_http_policy(
            command,
            label,
        ),
        "_uses_failed_receipt_policy": False,
        "_command_modes": _stage_command_mode_summary(name, command),
    }


def _receipt_summary_endpoint_evidence_kinds(
    receipt_summary: dict[str, Any] | None,
) -> set[str]:
    if receipt_summary is None:
        return set()
    return {
        receipt["receipt_kind"]
        for receipt in receipt_summary["receipts"]
        if receipt["endpoint_requires_insecure_http"]
    }


def _check_stage_receipt_kind_binding(
    path: str,
    stage_results: list[dict[str, Any]],
    receipt_summary: dict[str, Any] | None,
) -> None:
    required_receipt_kinds = {
        STAGE_RECEIPT_KINDS[stage["name"]]
        for stage in stage_results
        if stage["name"] in STAGE_RECEIPT_KINDS and not stage.get("_dry_run")
    }
    receipt_kinds = set(receipt_summary["receipt_kind"]) if receipt_summary else set()
    missing_receipt_kinds = sorted(required_receipt_kinds - receipt_kinds)
    if missing_receipt_kinds:
        raise EvidenceError(
            f"{path}.receipt_summary is missing receipt kinds for executed stages: "
            + ", ".join(missing_receipt_kinds)
        )
    unexecuted_receipt_kinds = sorted(receipt_kinds - required_receipt_kinds)
    if unexecuted_receipt_kinds:
        raise EvidenceError(
            f"{path}.receipt_summary contains receipt kinds for stages not executed"
        )


def _check_stage_adapter_receipt_path_binding(
    path: str,
    stage_stdout_receipts: dict[str, list[str]],
    receipt_summary: dict[str, Any] | None,
) -> None:
    if receipt_summary is None:
        return
    for stage_name, adapter_paths in stage_stdout_receipts.items():
        receipt_kind = STAGE_RECEIPT_KINDS[stage_name]
        verifier_paths = [
            receipt["path"]
            for receipt in receipt_summary["receipts"]
            if receipt["receipt_kind"] == receipt_kind
        ]
        if sorted(adapter_paths) != sorted(verifier_paths):
            raise EvidenceError(
                f"{path}.receipt_summary {stage_name} receipt paths do not match "
                f"{stage_name} stage stdout"
            )


def _check_verify_receipt_dir_scope(
    path: str,
    stage_results: list[dict[str, Any]],
    *,
    branch_label: str,
) -> None:
    verify_stage = next(
        (
            stage
            for stage in stage_results
            if stage["name"] == "verify"
        ),
        None,
    )
    if verify_stage is None:
        return
    verify_receipt_dirs = verify_stage["_receipt_dirs"]
    verify_receipt_files = verify_stage.get("_receipt_files", [])
    stage_receipt_dirs = {
        stage["receipt_dir"]
        for stage in stage_results
        if stage["name"] in STAGE_RECEIPT_KINDS and stage.get("receipt_dir") is not None
    }
    if set(verify_receipt_dirs) - stage_receipt_dirs:
        raise EvidenceError(
            f"{path}.{branch_label} verify command includes receipt_dir for stages not present"
        )
    if {
        receipt_file.rsplit("/", 1)[0] if "/" in receipt_file else "."
        for receipt_file in verify_receipt_files
    } - stage_receipt_dirs:
        raise EvidenceError(
            f"{path}.{branch_label} verify command includes receipt file for stages not present"
        )


def _check_verify_receipt_dir_coverage(
    path: str,
    stage_results: list[dict[str, Any]],
    *,
    branch_label: str,
) -> None:
    verify_stage = next(
        (
            stage
            for stage in stage_results
            if stage["name"] == "verify"
        ),
        None,
    )
    if verify_stage is None:
        return
    verify_receipt_dirs = set(verify_stage["_receipt_dirs"])
    for stage in stage_results:
        if stage["name"] not in STAGE_RECEIPT_KINDS or stage.get("_dry_run"):
            continue
        receipt_dir = stage.get("receipt_dir")
        if receipt_dir is not None and receipt_dir not in verify_receipt_dirs:
            raise EvidenceError(
                f"{path}.{branch_label} verify command does not include "
                f"{stage['name']} receipt_dir"
            )


def _check_stage_receipt_dirs_unique(
    path: str,
    stage_results: list[dict[str, Any]],
    *,
    branch_label: str,
) -> None:
    seen: dict[str, str] = {}
    for stage in stage_results:
        if stage["name"] not in STAGE_RECEIPT_KINDS:
            continue
        receipt_dir = stage.get("receipt_dir")
        if receipt_dir is None:
            continue
        previous_stage = seen.get(receipt_dir)
        if previous_stage is not None:
            raise EvidenceError(
                f"{path}.{branch_label} {stage['name']} receipt_dir duplicates "
                f"{previous_stage} receipt_dir"
            )
        seen[receipt_dir] = stage["name"]


def _receipt_summary_has_default_profile(
    receipt_summary: dict[str, Any] | None,
) -> bool:
    if receipt_summary is None:
        return False
    return any(
        receipt["receipt_kind"] == "iso-rail-gateway" and receipt.get("profile") is None
        for receipt in receipt_summary["receipts"]
    )


def _receipt_summary_has_legacy_colr007(
    receipt_summary: dict[str, Any] | None,
) -> bool:
    if receipt_summary is None:
        return False
    return any(
        receipt["receipt_kind"] == "iso-rail-gateway"
        and receipt.get("message_type") in LEGACY_RAIL_MESSAGE_TYPES
        for receipt in receipt_summary["receipts"]
    )


def _check_rail_receipt_policy_binding(
    path: str,
    stage_results: list[dict[str, Any]],
    receipt_summary: dict[str, Any] | None,
) -> None:
    rail_stage = next(
        (
            stage
            for stage in stage_results
            if stage["name"] == "rail" and not stage.get("_dry_run")
        ),
        None,
    )
    if rail_stage is None:
        return

    receipt_has_default_profile = _receipt_summary_has_default_profile(receipt_summary)
    receipt_has_legacy_colr007 = _receipt_summary_has_legacy_colr007(receipt_summary)
    rail_uses_default_profile = rail_stage["_uses_default_profile_policy"]
    rail_uses_legacy_colr007 = rail_stage["_uses_legacy_colr007_policy"]

    if receipt_has_default_profile and not rail_uses_default_profile:
        raise EvidenceError(
            f"{path}.receipt_summary records default rail profile but rail command "
            "omitted --allow-default-profile"
        )
    if rail_uses_default_profile and not receipt_has_default_profile:
        raise EvidenceError(
            f"{path}.stages rail command used --allow-default-profile but "
            "receipt_summary has no default-profile rail receipt"
        )
    if receipt_has_legacy_colr007 and not rail_uses_legacy_colr007:
        raise EvidenceError(
            f"{path}.receipt_summary records legacy colr.007 but rail command "
            "omitted --allow-legacy-colr007"
        )
    if rail_uses_legacy_colr007 and not receipt_has_legacy_colr007:
        raise EvidenceError(
            f"{path}.stages rail command used --allow-legacy-colr007 but "
            "receipt_summary has no legacy colr.007 rail receipt"
        )


def _check_notary_receipt_anchor_policy_binding(
    path: str,
    stage_results: list[dict[str, Any]],
    receipt_summary: dict[str, Any] | None,
) -> None:
    notary_stage = next(
        (
            stage
            for stage in stage_results
            if stage["name"] == "notary" and not stage.get("_dry_run")
        ),
        None,
    )
    if notary_stage is None or receipt_summary is None:
        return

    notary_uses_all = _command_has_flag(notary_stage["_command"], "--all")
    notary_receipts = [
        receipt
        for receipt in receipt_summary["receipts"]
        if receipt["receipt_kind"] == "iso-audit-notary"
    ]
    if not notary_receipts:
        return
    has_latest_anchor = any(
        receipt["anchor_path"].split("/")[-1] == LATEST_ANCHOR_FILE
        for receipt in notary_receipts
    )
    has_digest_addressed_anchor = any(
        receipt["anchor_path"].split("/")[-1] != LATEST_ANCHOR_FILE
        for receipt in notary_receipts
    )
    if notary_uses_all and has_latest_anchor:
        raise EvidenceError(
            f"{path}.receipt_summary records latest notary anchor but notary "
            "command used --all"
        )
    if not notary_uses_all and has_digest_addressed_anchor:
        raise EvidenceError(
            f"{path}.receipt_summary records digest-addressed notary anchor "
            "but notary command omitted --all"
        )


def verify_canary_summary(
    path: Path,
    args: argparse.Namespace,
    *,
    display_label: str | None = None,
) -> dict[str, Any]:
    """Verify one archived canary summary and return compact evidence metadata."""

    label = display_label or "canary summary"
    if _receipt_path_is_repository_fixture(str(path)):
        raise EvidenceError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )
    summary = _require_object(_load_json(path, display_label=label), label)
    digest = _require_summary_digest(summary, label)
    _reject_unknown_keys(summary, CANARY_SUMMARY_KEYS, label)
    _check_no_secret_material(summary, label)
    version = summary.get("version")
    if type(version) is not int or version != CANARY_SUMMARY_VERSION:
        raise EvidenceError(f"{label}.version must be {CANARY_SUMMARY_VERSION}")

    provider = _required_context_string(summary, "provider", label)
    environment = _required_context_string(summary, "environment", label)
    config_path = _validate_config_path(
        _required_string(summary, "config_path", label),
        f"{label}.config_path",
    )
    started_at_raw, started_at = _required_timestamp(summary, "started_at", label)
    finished_at_raw, finished_at = _required_timestamp(summary, "finished_at", label)
    if finished_at < started_at:
        raise EvidenceError(f"{label}.finished_at must not be before started_at")
    _reject_stale_timestamp(
        finished_at,
        max_age_days=args.max_canary_age_days,
        label=f"{label}.finished_at",
    )
    if args.provider is not None and provider != args.provider:
        raise EvidenceError(f"{label}.provider does not match expected provider")
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(f"{label}.environment does not match expected environment")

    ok = _required_bool(summary, "ok", label)
    if not ok:
        raise EvidenceError(f"{label} is not an ok canary summary")
    plan_only = _required_bool(summary, "plan_only", label)
    if plan_only and not args.allow_plan_only:
        raise EvidenceError(f"{label} is plan-only evidence")
    policy = _require_object(summary.get("policy"), f"{label}.policy")
    _reject_unknown_keys(policy, CANARY_POLICY_KEYS, f"{label}.policy")
    require_explicit_policy = _required_bool(
        policy,
        "require_explicit_policy",
        f"{label}.policy",
    )
    if not require_explicit_policy:
        raise EvidenceError(f"{label} was not produced with --require-explicit-policy")

    stage_results: list[dict[str, Any]] = []
    executed_stage_objects: list[dict[str, Any]] = []
    if plan_only:
        if "stages" in summary:
            raise EvidenceError(f"{label}.stages must be omitted for plan-only evidence")
        stages = _require_list(summary.get("planned_stages"), f"{label}.planned_stages")
        planned_stage_results = [
            _planned_stage_summary(
                _require_object(stage, f"{label}.planned_stages[{offset}]"),
                f"{label}.planned_stages[{offset}]",
                args,
            )
            for offset, stage in enumerate(stages)
        ]
        stage_names = [stage["name"] for stage in planned_stage_results]
    else:
        if "planned_stages" in summary:
            raise EvidenceError(f"{label}.planned_stages must be omitted for executed evidence")
        stages = _require_list(summary.get("stages"), f"{label}.stages")
        executed_stage_objects = [
            _require_object(stage, f"{label}.stages[{offset}]")
            for offset, stage in enumerate(stages)
        ]
        stage_results = [
            _stage_summary(
                stage,
                f"{label}.stages[{offset}]",
                args,
                canary_started_at=started_at,
                canary_finished_at=finished_at,
            )
            for offset, stage in enumerate(executed_stage_objects)
        ]
        stage_names = [stage["name"] for stage in stage_results]

    if len(stage_names) != len(set(stage_names)):
        raise EvidenceError(f"{label} contains duplicate canary stages")
    _check_canary_stage_sequence(stage_names, label)
    previous_finished_at: dt.datetime | None = None
    for offset, stage in enumerate(stage_results):
        if previous_finished_at is not None and stage["_started_at"] < previous_finished_at:
            raise EvidenceError(
                f"{label}.stages[{offset}].started_at must not be before previous stage finished_at"
            )
        previous_finished_at = stage["_finished_at"]
    stage_name_set = set(stage_names)
    if args.allow_partial_canary:
        if "verify" not in stage_name_set:
            raise EvidenceError(f"{label} is missing verify stage")
        if not ({"rail", "notary"} & stage_name_set):
            raise EvidenceError(f"{label} must include rail or notary stage")
    else:
        missing = sorted(REQUIRED_CANARY_STAGES - stage_name_set)
        if missing:
            raise EvidenceError(
                f"{label} is missing required canary stages: {', '.join(missing)}"
            )
    stage_branch_label = "planned_stages" if plan_only else "stages"
    stage_results_for_receipts = planned_stage_results if plan_only else stage_results
    _check_stage_receipt_dirs_unique(
        label,
        stage_results_for_receipts,
        branch_label=stage_branch_label,
    )
    _check_verify_receipt_dir_coverage(
        label,
        stage_results_for_receipts,
        branch_label=stage_branch_label,
    )
    _check_verify_receipt_dir_scope(
        label,
        stage_results_for_receipts,
        branch_label=stage_branch_label,
    )
    stage_stdout_receipts: dict[str, list[str]] = {}
    for offset, (stage, stage_result) in enumerate(
        zip(executed_stage_objects, stage_results)
    ):
        if stage_result["name"] in {"rail", "notary"} and not stage_result.get("_dry_run"):
            receipt_dir = stage_result.get("receipt_dir")
            if receipt_dir is not None:
                adapter_stdout = _check_stage_adapter_stdout(
                    stage_result["name"],
                    stage,
                    f"{label}.stages[{offset}]",
                    receipt_dir,
                    stage_result["_command"],
                )
                stage_stdout_receipts[stage_result["name"]] = adapter_stdout[
                    "receipts"
                ]
                if stage_result["name"] == "notary":
                    stage_result["_command_modes"][
                        "notary_published_anchor_count"
                    ] = adapter_stdout["notary_published_anchor_count"]
                if stage_result["name"] == "rail":
                    stage_result["_command_modes"][
                        "rail_submitted_message_count"
                    ] = adapter_stdout["rail_submitted_message_count"]
        elif stage_result["name"] == "verify" and "receipt_summary" in stage_result:
            _check_receipt_stdout_selector_binding(
                stage_result["_receipt_dirs"],
                stage_result["_receipt_files"],
                stage_result["receipt_summary"],
                f"{label}.stages[{offset}]",
            )
    receipt_summary = next(
        (
            stage["receipt_summary"]
            for stage in stage_results
            if stage["name"] == "verify" and "receipt_summary" in stage
        ),
        None,
    )
    required_endpoint_evidence_kinds = {
        stage["_requires_insecure_http_receipt_kind"]
        for stage in stage_results
        if stage.get("_requires_insecure_http_receipt_kind") is not None
    }
    endpoint_evidence_kinds = _receipt_summary_endpoint_evidence_kinds(receipt_summary)
    missing_endpoint_evidence_kinds = sorted(
        required_endpoint_evidence_kinds - endpoint_evidence_kinds
    )
    if missing_endpoint_evidence_kinds:
        raise EvidenceError(
            f"{label}.receipt_summary is missing endpoint_requires_insecure_http "
            "evidence for insecure command receipt kinds: "
            + ", ".join(missing_endpoint_evidence_kinds)
        )
    _check_stage_receipt_kind_binding(label, stage_results, receipt_summary)
    _check_stage_adapter_receipt_path_binding(
        label,
        stage_stdout_receipts,
        receipt_summary,
    )
    _check_rail_receipt_policy_binding(label, stage_results, receipt_summary)
    _check_notary_receipt_anchor_policy_binding(label, stage_results, receipt_summary)
    policy_stage_results = planned_stage_results if plan_only else stage_results

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
        "stage_dry_run": [
            bool(stage.get("_dry_run")) for stage in policy_stage_results
        ],
        "stage_command_modes": [
            stage["_command_modes"] for stage in policy_stage_results
        ],
        "stage_windows": [
            {
                "name": stage["name"],
                "started_at": stage["started_at"],
                "finished_at": stage["finished_at"],
            }
            for stage in stage_results
        ],
        "receipt_summary": receipt_summary,
        "_uses_dry_run_policy": any(
            stage["_uses_dry_run_policy"] for stage in policy_stage_results
        ),
        "_uses_insecure_http_policy": any(
            stage["_uses_insecure_http_policy"] for stage in policy_stage_results
        ),
        "_uses_failed_receipt_policy": any(
            stage["_uses_failed_receipt_policy"] for stage in policy_stage_results
        ),
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
    if _contains_control_character(url):
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
    if any(ord(ch) > 0x7E for ch in raw_host):
        raise EvidenceError(f"{label} host must use printable ASCII")
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
    if any(ord(ch) > 0x7E for ch in path):
        raise EvidenceError(f"{label} path must use printable ASCII")
    if "\\" in path:
        raise EvidenceError(f"{label} path must use forward slashes")
    if ";" in path:
        raise EvidenceError(f"{label} path must not contain semicolon parameters")
    if any(token in path for token in (":", "@", "[", "]")):
        raise EvidenceError(f"{label} path must not contain URL delimiter characters")
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
    if re.search(r"%[89a-f][0-9a-f]", lowered):
        raise EvidenceError(f"{label} path must not contain percent-encoded non-ASCII bytes")


def _check_https_url(url: str, label: str, *, allow_insecure_http: bool) -> None:
    _check_clean_http_url(
        url,
        label,
        allow_insecure_http=allow_insecure_http,
        reject_local_hosts=True,
    )


def _trust_source_placeholder_forms(value: str) -> tuple[str, str, str]:
    folded_forms = []
    separated_forms = []
    squeezed_forms = []
    for candidate in (
        value,
        unicodedata.normalize("NFKC", value),
        unicodedata.normalize("NFKD", value),
    ):
        folded = candidate.casefold()
        folded_forms.append(folded)
        separated_forms.append(" ".join(re.sub(r"[^a-z0-9]+", " ", folded).split()))
        squeezed_forms.append("".join(ch for ch in folded if ch.isalnum()))
    return (
        " ".join(dict.fromkeys(folded_forms)),
        " ".join(dict.fromkeys(separated_forms)),
        " ".join(dict.fromkeys(squeezed_forms)),
    )


def _trust_source_text_is_placeholder(value: str) -> bool:
    folded, separated, squeezed = _trust_source_placeholder_forms(value)
    for marker in PLACEHOLDER_TRUST_SOURCE_MARKERS:
        marker_folded, marker_separated, marker_squeezed = (
            _trust_source_placeholder_forms(marker)
        )
        if (
            marker_folded in folded
            or marker_separated in separated
            or marker_squeezed in squeezed
        ):
            return True
    return False


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
    value = _plain_text(value, label)
    if len(value) > MAX_TIMESTAMP_CHARS:
        raise EvidenceError(
            f"{label} must be no longer than {MAX_TIMESTAMP_CHARS} characters"
        )
    if any(
        ord(ch) < 0x20 or ord(ch) == 0x7F or unicodedata.category(ch) == "Cf"
        for ch in value
    ):
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
    if CANONICAL_TIMESTAMP_RE.fullmatch(text) is None or text.endswith("-00:00"):
        raise EvidenceError(f"{label} must use a canonical ISO 8601 timestamp")
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


def _trust_source_requires_insecure_override(source: dict[str, str] | None) -> bool:
    if source is None:
        return False
    return _url_requires_insecure_http_override(urllib.parse.urlparse(source["url"]))


def _check_trust_bundle(
    bundle: dict[str, Any],
    label: str,
    args: argparse.Namespace,
) -> dict[str, Any]:
    _reject_unknown_keys(bundle, TRUST_BUNDLE_KEYS, label)
    bundle_path = _validate_trust_bundle_path(
        _required_string(bundle, "path", label),
        f"{label}.path",
    )
    profile_id = _required_profile_id(bundle, "profile_id", label)
    rail = _required_rail(bundle, "rail", label)
    environment = _required_context_string(bundle, "environment", label)
    if args.environment is not None and environment != args.environment:
        raise EvidenceError(
            f"{label}.environment does not match expected environment"
        )
    policy = _required_string(bundle, "embedded_signature_policy", label)
    _reject_overlong_trust_policy(policy, f"{label}.embedded_signature_policy")
    _reject_non_ascii_context(policy, f"{label}.embedded_signature_policy")
    _reject_secret_looking_identifier(policy, f"{label}.embedded_signature_policy")
    if policy not in TRUST_SIGNATURE_POLICIES:
        raise EvidenceError(f"{label}.embedded_signature_policy is unsupported")
    if policy != REQUIRE_VERIFIED and not args.allow_record_only_trust:
        raise EvidenceError(
            f"{label}.embedded_signature_policy does not require verified signatures"
        )

    source_summary: dict[str, str] | None = None
    bundle_sha256 = bundle.get("bundle_sha256")
    if isinstance(bundle_sha256, str):
        _reject_secret_looking_identifier(bundle_sha256, f"{label}.bundle_sha256")
    if not _is_lower_sha256(bundle_sha256):
        raise EvidenceError(f"{label}.bundle_sha256 must be a canonical SHA-256")
    _reject_all_zero_sha256(bundle_sha256, f"{label}.bundle_sha256")
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
        _reject_overlong_trust_source_text(authority, f"{label}.source.authority")
        _reject_overlong_trust_source_text(version, f"{label}.source.version")
        _reject_non_ascii_context(authority, f"{label}.source.authority")
        _reject_non_ascii_context(version, f"{label}.source.version")
        _reject_secret_looking_identifier(authority, f"{label}.source.authority")
        _reject_secret_looking_identifier(version, f"{label}.source.version")
        url = source_obj.get("url")
        if not isinstance(url, str):
            raise EvidenceError(f"{label}.source.url must be recorded")
        url = _plain_text(url, f"{label}.source.url")
        if not url.strip():
            raise EvidenceError(f"{label}.source.url must be recorded")
        if _contains_control_character(url):
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
        if _contains_control_character(retrieved_at):
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
    override_id = _required_profile_id(
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
    _reject_overlong_trust_policy(
        override_policy,
        f"{label}.profile_overrides.embedded_signature_policy",
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
    _reject_sha256_overlap(
        override_public_pins + override_legacy_public_pins,
        override_anchor_pins + override_legacy_anchor_pins + override_revoked_pins,
        f"{label}.profile_overrides public-key/certificate SHA-256 pins",
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
        der_kind=TRUST_DER_KIND_CRL,
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
        der_kind=TRUST_DER_KIND_OCSP,
    )
    if len(ocsp_der) != x509_ocsp_response_count:
        raise EvidenceError(f"{label}.profile_overrides OCSP DER count does not match material")
    _require_override_der_matches_summary(
        ocsp_der,
        ocsp_der_entries,
        f"{label}.profile_overrides.x509_ocsp_response_der_base64",
        f"{label}.x509_ocsp_responses",
    )
    _reject_sha256_overlap(
        override_public_pins
        + override_legacy_public_pins
        + override_anchor_pins
        + override_legacy_anchor_pins
        + override_revoked_pins,
        list(crl_der_entries) + list(ocsp_der_entries),
        f"{label}.profile_overrides trust pin/revocation DER SHA-256 roles",
    )

    return {
        "path": bundle_path,
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


def _reject_trust_digest_role_reuse(
    profile_json_sha256: str | None,
    bundle_summaries: list[dict[str, Any]],
    label: str,
) -> None:
    bundle_digests: dict[str, str] = {}
    der_digests: dict[str, str] = {}
    der_roles = (
        "x509_trust_anchor_der",
        "revoked_certificate_der",
        "x509_crl_der",
        "x509_ocsp_response_der",
    )
    for offset, bundle in enumerate(bundle_summaries):
        bundle_label = f"{label}.bundles[{offset}]"
        bundle_digests[bundle["bundle_sha256"]] = f"{bundle_label}.bundle_sha256"
        for der_role in der_roles:
            for der_offset, entry in enumerate(bundle[der_role]):
                der_digests[str(entry["sha256"])] = (
                    f"{bundle_label}.{der_role}[{der_offset}].sha256"
                )

    if profile_json_sha256 is not None:
        if profile_json_sha256 in bundle_digests:
            raise EvidenceError(
                f"{label}.profile_json_sha256 must not reuse "
                f"{bundle_digests[profile_json_sha256]}"
            )
        if profile_json_sha256 in der_digests:
            raise EvidenceError(
                f"{label}.profile_json_sha256 must not reuse "
                f"{der_digests[profile_json_sha256]}"
            )

    for bundle_digest, bundle_label in bundle_digests.items():
        if bundle_digest in der_digests:
            raise EvidenceError(
                f"{bundle_label} must not reuse {der_digests[bundle_digest]}"
            )


def verify_trust_summary(
    path: Path,
    args: argparse.Namespace,
    *,
    display_label: str | None = None,
) -> dict[str, Any]:
    """Verify one archived trust-bundle summary and return compact metadata."""

    label = display_label or "trust summary"
    if _receipt_path_is_repository_fixture(str(path)):
        raise EvidenceError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )
    summary = _require_object(_load_json(path, display_label=label), label)
    digest = _require_summary_digest(summary, label)
    _reject_unknown_keys(summary, TRUST_SUMMARY_KEYS, label)
    _check_no_secret_material(summary, label)
    version = summary.get("version")
    if type(version) is not int or version != TRUST_SUMMARY_VERSION:
        raise EvidenceError(f"{label}.version must be {TRUST_SUMMARY_VERSION}")
    verified_at_raw, verified_at = _required_timestamp(summary, "verified_at", label)
    _reject_stale_timestamp(
        verified_at,
        max_age_days=args.max_trust_age_days,
        label=f"{label}.verified_at",
    )

    allow_synthetic_der = _required_bool(summary, "allow_synthetic_der", label)
    allow_record_only = _required_bool(summary, "allow_record_only", label)
    allow_insecure_source_url = _required_bool(summary, "allow_insecure_source_url", label)
    profile_json_emitted = _required_bool(summary, "profile_json_emitted", label)
    profile_json_emittable = _required_bool(summary, "profile_json_emittable", label)
    if "max_source_age_days" not in summary:
        raise EvidenceError(f"{label}.max_source_age_days must be recorded")
    if summary["max_source_age_days"] is None:
        max_source_age_days = None
    else:
        max_source_age_days = _required_positive_int_field(
            summary,
            "max_source_age_days",
            label,
        )
    if profile_json_emitted:
        profile_json_sha256 = _required_nonzero_sha256(
            summary,
            "profile_json_sha256",
            label,
        )
    else:
        if "profile_json_sha256" not in summary:
            raise EvidenceError(f"{label}.profile_json_sha256 must be null when profile JSON was not emitted")
        if summary["profile_json_sha256"] is not None:
            raise EvidenceError(f"{label}.profile_json_sha256 must be null when profile JSON was not emitted")
        profile_json_sha256 = None
    if allow_synthetic_der and not args.allow_synthetic_trust:
        raise EvidenceError(f"{label} was verified with --allow-synthetic-der")
    if allow_record_only and not args.allow_record_only_trust:
        raise EvidenceError(f"{label} was verified with --allow-record-only")
    if allow_insecure_source_url and not args.allow_insecure_http:
        raise EvidenceError(f"{label} was verified with --allow-insecure-source-url")
    if profile_json_emittable:
        if max_source_age_days is None:
            raise EvidenceError(f"{label}.max_source_age_days must be a positive integer")
        if max_source_age_days > args.max_trust_source_age_days:
            raise EvidenceError(
                f"{label}.max_source_age_days is weaker than "
                "--max-trust-source-age-days"
            )
    if not profile_json_emitted and not args.allow_profile_json_not_emitted:
        raise EvidenceError(f"{label} did not emit profile JSON")

    verified_bundles = summary.get("verified_bundles")
    if isinstance(verified_bundles, bool) or not isinstance(verified_bundles, int) or verified_bundles <= 0:
        raise EvidenceError(f"{label}.verified_bundles must be a positive integer")
    bundles = _require_list(summary.get("bundles"), f"{label}.bundles")
    if len(bundles) != verified_bundles:
        raise EvidenceError(f"{label}.bundles length does not match verified_bundles")
    bundle_objects = [
        _require_object(bundle, f"{label}.bundles[{offset}]")
        for offset, bundle in enumerate(bundles)
    ]
    bundle_summaries = [
        _check_trust_bundle(
            bundle,
            f"{label}.bundles[{offset}]",
            args,
        )
        for offset, bundle in enumerate(bundle_objects)
    ]
    if any(
        bundle["embedded_signature_policy"] != REQUIRE_VERIFIED
        for bundle in bundle_summaries
    ) and not allow_record_only:
        raise EvidenceError(
            f"{label}.allow_record_only must be true when a bundle records "
            "a non-production embedded_signature_policy"
        )
    if any(
        _trust_source_requires_insecure_override(bundle["source"])
        for bundle in bundle_summaries
    ) and not allow_insecure_source_url:
        raise EvidenceError(
            f"{label}.allow_insecure_source_url must be true when a bundle records "
            "an http:// or local/private source URL"
        )
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
        raise EvidenceError(f"{label} cannot emit production profile JSON")
    computed_profile_json_emittable = _computed_profile_json_emittable(
        allow_synthetic_der=allow_synthetic_der,
        allow_record_only=allow_record_only,
        allow_insecure_source_url=allow_insecure_source_url,
        max_source_age_days=max_source_age_days,
        bundle_summaries=bundle_summaries,
    )
    if profile_json_emittable != computed_profile_json_emittable:
        raise EvidenceError(
            f"{label}.profile_json_emittable does not match trust source policy"
        )
    if profile_json_emitted and not profile_json_emittable:
        raise EvidenceError(
            f"{label}.profile_json_emitted cannot be true when profile_json_emittable is false"
        )
    if profile_json_emitted and not computed_profile_json_emittable:
        raise EvidenceError(
            f"{label}.profile_json_emitted cannot be true when trust source policy is not emittable"
        )
    if profile_json_emitted:
        profile_config = [bundle["profile_overrides"] for bundle in bundle_objects]
        expected_profile_text = (
            json.dumps(profile_config, allow_nan=False, indent=2, sort_keys=True) + "\n"
        )
        expected_profile_sha256 = sha256_hex(expected_profile_text.encode("utf-8"))
        if profile_json_sha256 != expected_profile_sha256:
            raise EvidenceError(
                f"{label}.profile_json_sha256 does not match archived profile_overrides"
            )
    _reject_trust_digest_role_reuse(profile_json_sha256, bundle_summaries, label)
    bundle_order_keys = [
        _trust_bundle_summary_order_key(bundle) for bundle in bundle_summaries
    ]
    if bundle_order_keys != sorted(bundle_order_keys):
        raise EvidenceError(
            f"{label}.bundles must be sorted by profile_id, path, and bundle_sha256"
        )
    seen_profile_ids: dict[str, int] = {}
    seen_bundle_paths: dict[str, int] = {}
    seen_bundle_digests: dict[str, int] = {}
    for offset, bundle in enumerate(bundle_summaries):
        profile_id = bundle["profile_id"]
        if profile_id in seen_profile_ids:
            raise EvidenceError(
                f"{label}.bundles[{offset}].profile_id duplicates "
                f"{label}.bundles[{seen_profile_ids[profile_id]}].profile_id"
            )
        seen_profile_ids[profile_id] = offset
        bundle_path = bundle["path"]
        if bundle_path in seen_bundle_paths:
            raise EvidenceError(
                f"{label}.bundles[{offset}].path duplicates "
                f"{label}.bundles[{seen_bundle_paths[bundle_path]}].path"
            )
        seen_bundle_paths[bundle_path] = offset
        bundle_sha256 = bundle["bundle_sha256"]
        if bundle_sha256 in seen_bundle_digests:
            raise EvidenceError(
                f"{label}.bundles[{offset}].bundle_sha256 duplicates "
                f"{label}.bundles[{seen_bundle_digests[bundle_sha256]}].bundle_sha256"
            )
        seen_bundle_digests[bundle_sha256] = offset
    if allow_record_only and not any(
        bundle["embedded_signature_policy"] != REQUIRE_VERIFIED
        for bundle in bundle_summaries
    ):
        raise EvidenceError(
            f"{label}.allow_record_only requires at least one non-production "
            "embedded_signature_policy"
        )
    if allow_insecure_source_url and not any(
        _trust_source_requires_insecure_override(bundle["source"])
        for bundle in bundle_summaries
    ):
        raise EvidenceError(
            f"{label}.allow_insecure_source_url requires at least one http:// "
            "or local/private source URL"
        )
    compact_profiles = sorted(
        bundle_summaries,
        key=lambda bundle: bundle["profile_id"],
    )
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
        "profiles": compact_profiles,
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
        detail = _receipt_verifier_stderr_detail(stderr)
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
    if stderr.strip():
        detail = _receipt_verifier_stderr_detail(stderr)
        raise EvidenceError(
            "receipt verifier emitted stderr on successful verification: " + detail
        )
    try:
        receipt_summary = json.loads(
            stdout,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_int=_parse_canonical_json_int,
            parse_float=_parse_canonical_json_float,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise EvidenceError("receipt verifier emitted invalid JSON") from error
    except RecursionError as error:
        raise EvidenceError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(receipt_summary)
    receipt_obj = _require_object(receipt_summary, "receipt verifier summary")
    return _verify_receipt_verifier_summary(
        receipt_obj,
        "receipt verifier summary",
        args,
        allow_partial_receipt_kinds=args.allow_partial_canary or args.allow_dry_run,
    )


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
                    f"[{receipt_offset}].receipt_sha256"
                )
            direct_kind = direct_receipt["receipt_kind"]
            if direct_kind != receipt["receipt_kind"]:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 to "
                    "a receipt kind that does not match canary receipt kind"
                )
            direct_path_name = Path(direct_receipt["path"]).name
            canary_path_name = Path(receipt["path"]).name
            if direct_path_name != canary_path_name:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 to "
                    "a receipt filename that does not match canary receipt filename"
                )
            direct_metadata = _receipt_entry_content_metadata(direct_receipt)
            canary_metadata = _receipt_entry_content_metadata(receipt)
            if direct_metadata != canary_metadata:
                raise EvidenceError(
                    "direct receipt archive verification binds "
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].receipt_sha256 to "
                    "metadata that does not match canary receipt metadata"
                )
    for receipt_offset, receipt in enumerate(receipt_summary["receipts"]):
        receipt_sha256 = receipt["receipt_sha256"]
        if receipt_sha256 not in canary_receipt_kinds_by_digest:
            raise EvidenceError(
                "direct receipt archive verification includes unreferenced "
                f"receipt_verification.receipts[{receipt_offset}].receipt_sha256"
            )


def _compact_receipt_summaries(
    canaries: list[dict[str, Any]],
    receipt_summary: dict[str, Any] | None,
) -> list[dict[str, Any]]:
    """Return direct and canary receipt summaries already verified into compact form."""

    summaries: list[dict[str, Any]] = []
    if receipt_summary is not None:
        summaries.append(receipt_summary)
    for canary in canaries:
        canary_receipt_summary = canary.get("receipt_summary")
        if isinstance(canary_receipt_summary, dict):
            summaries.append(canary_receipt_summary)
    return summaries


def _receipt_summaries_have_legacy_colr007(
    receipt_summaries: list[dict[str, Any]],
) -> bool:
    return any(
        receipt.get("receipt_kind") == "iso-rail-gateway"
        and receipt.get("message_type") in LEGACY_RAIL_MESSAGE_TYPES
        for summary in receipt_summaries
        for receipt in summary["receipts"]
    )


def _receipt_summaries_have_default_profile(
    receipt_summaries: list[dict[str, Any]],
) -> bool:
    return any(
        receipt.get("receipt_kind") == "iso-rail-gateway"
        and receipt.get("profile") is None
        for summary in receipt_summaries
        for receipt in summary["receipts"]
    )


def _receipt_summaries_have_source_file_gap(
    receipt_summaries: list[dict[str, Any]],
) -> bool:
    return any(not summary["require_source_files"] for summary in receipt_summaries)


def _plain_public_json_value(value: Any) -> Any:
    if type(value) is dict:
        output: dict[str, Any] = {}
        for key, child in value.items():
            if type(key) is not str:
                return "unsupported"
            output[key] = _plain_public_json_value(child)
        return output
    if type(value) is list:
        return [_plain_public_json_value(item) for item in value]
    if (
        value is None
        or type(value) is str
        or type(value) is bool
        or type(value) is int
    ):
        return value
    if type(value) is float:
        return value if math.isfinite(value) else "unsupported"
    return "unsupported"


def _public_json_object_without_private_fields(summary: Any) -> dict[str, Any]:
    if type(summary) is not dict:
        return {}
    output: dict[str, Any] = {}
    for key, value in summary.items():
        if type(key) is not str or key.startswith("_"):
            continue
        output[key] = _plain_public_json_value(value)
    return output


def _public_canary_summary(canary: dict[str, Any]) -> dict[str, Any]:
    return _public_json_object_without_private_fields(canary)


def _compact_summary_order_key(summary: dict[str, Any]) -> tuple[str, str]:
    return (summary["path"], summary["summary_sha256"])


def _reject_cross_canary_receipt_reuse(canaries: list[dict[str, Any]]) -> None:
    """Reject receipt path or digest reuse across distinct canary summaries."""

    seen_paths: dict[str, tuple[int, int]] = {}
    seen_digests: dict[str, tuple[int, int]] = {}
    source_material_checks: tuple[tuple[str, str], ...] = (
        ("source_path", "source_path"),
        ("payload_sha256", "payload_sha256"),
        ("rail_message_id", "rail_message_id"),
        ("anchor_path", "anchor_path"),
        ("anchor_sha256", "anchor_sha256"),
        ("store_dir", "store_dir"),
        ("index_path", "index_path"),
        ("index_sha256", "index_sha256"),
    )
    seen_source_material: dict[str, dict[str, tuple[int, int]]] = {
        field: {} for field, _label in source_material_checks
    }
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
            for field, field_label in source_material_checks:
                value = receipt.get(field)
                if not isinstance(value, str):
                    continue
                seen_for_field = seen_source_material[field]
                previous = seen_for_field.get(value)
                if previous is None:
                    seen_for_field[value] = (canary_offset, receipt_offset)
                    continue
                first_canary, first_receipt = previous
                if first_canary == canary_offset:
                    continue
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].{field_label} duplicates "
                    f"canary_summaries[{first_canary}].receipt_summary.receipts"
                    f"[{first_receipt}].{field_label}"
                )


def _reject_cross_trust_profile_reuse(trusts: list[dict[str, Any]]) -> None:
    """Reject trust profile material reused across distinct trust summaries."""

    seen_profile_json_digests: dict[str, int] = {}
    seen_profile_ids: dict[str, tuple[int, int]] = {}
    seen_bundle_paths: dict[str, tuple[int, int]] = {}
    seen_bundle_digests: dict[str, tuple[int, int]] = {}
    for trust_offset, trust in enumerate(trusts):
        profile_json_sha256 = trust.get("profile_json_sha256")
        if isinstance(profile_json_sha256, str):
            if profile_json_sha256 in seen_profile_json_digests:
                first_trust = seen_profile_json_digests[profile_json_sha256]
                raise EvidenceError(
                    f"trust_summaries[{trust_offset}].profile_json_sha256 "
                    f"duplicates trust_summaries[{first_trust}].profile_json_sha256"
                )
            seen_profile_json_digests[profile_json_sha256] = trust_offset
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
            bundle_path = profile["path"]
            if bundle_path in seen_bundle_paths:
                first_trust, first_profile = seen_bundle_paths[bundle_path]
                raise EvidenceError(
                    f"trust_summaries[{trust_offset}].profiles[{profile_offset}].path "
                    f"duplicates trust_summaries[{first_trust}].profiles"
                    f"[{first_profile}].path"
                )
            seen_bundle_paths[bundle_path] = (trust_offset, profile_offset)
            bundle_sha256 = profile["bundle_sha256"]
            if bundle_sha256 in seen_bundle_digests:
                first_trust, first_profile = seen_bundle_digests[bundle_sha256]
                raise EvidenceError(
                    f"trust_summaries[{trust_offset}].profiles[{profile_offset}].bundle_sha256 "
                    f"duplicates trust_summaries[{first_trust}].profiles"
                    f"[{first_profile}].bundle_sha256"
                )
            seen_bundle_digests[bundle_sha256] = (trust_offset, profile_offset)


def _reject_compact_json_artifact_path_role_reuse(
    canaries: list[dict[str, Any]],
    trusts: list[dict[str, Any]],
    receipt_summary: dict[str, Any] | None = None,
) -> None:
    """Reject compact summary paths reused as config, receipt, or trust-bundle paths."""

    material_paths: dict[str, str] = {}
    for canary_offset, canary in enumerate(canaries):
        material_paths.setdefault(
            canary["config_path"],
            f"canary_summaries[{canary_offset}].config_path",
        )
        canary_receipt_summary = canary.get("receipt_summary")
        if isinstance(canary_receipt_summary, dict):
            for receipt_offset, receipt in enumerate(
                canary_receipt_summary["receipts"]
            ):
                receipt_path = receipt.get("path")
                if isinstance(receipt_path, str):
                    material_paths.setdefault(
                        receipt_path,
                        (
                            f"canary_summaries[{canary_offset}].receipt_summary"
                            f".receipts[{receipt_offset}].path"
                        ),
                    )
    for trust_offset, trust in enumerate(trusts):
        for profile_offset, profile in enumerate(trust["profiles"]):
            material_paths.setdefault(
                profile["path"],
                f"trust_summaries[{trust_offset}].profiles[{profile_offset}].path",
            )
    if isinstance(receipt_summary, dict):
        for receipt_offset, receipt in enumerate(receipt_summary["receipts"]):
            receipt_path = receipt.get("path")
            if isinstance(receipt_path, str):
                material_paths.setdefault(
                    receipt_path,
                    f"receipt_verification.receipts[{receipt_offset}].path",
                )

    for canary_offset, canary in enumerate(canaries):
        material_label = material_paths.get(canary["path"])
        if material_label is not None:
            raise EvidenceError(
                f"canary_summaries[{canary_offset}].path duplicates {material_label}"
            )
    for trust_offset, trust in enumerate(trusts):
        material_label = material_paths.get(trust["path"])
        if material_label is not None:
            raise EvidenceError(
                f"trust_summaries[{trust_offset}].path duplicates {material_label}"
            )


def _trusts_have_missing_source(trusts: list[dict[str, Any]]) -> bool:
    return any(
        profile.get("source") is None
        for trust in trusts
        for profile in trust["profiles"]
    )


def _reject_canary_rail_receipts_without_trust(
    canaries: list[dict[str, Any]],
    trusts: list[dict[str, Any]],
    args: argparse.Namespace,
) -> None:
    """Reject canary rail receipts that lack matching trust material."""

    trusted_profile_rails_by_environment: dict[tuple[str, str], set[str]] = {}
    for trust in trusts:
        for profile in trust["profiles"]:
            key = (profile["profile_id"], profile["environment"])
            trusted_profile_rails_by_environment.setdefault(key, set()).add(
                profile["rail"]
            )
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
            if profile_id is None:
                if args.default_rail_profile is None:
                    raise EvidenceError(
                        f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                        f"[{receipt_offset}].profile uses default rail profile without "
                        "--default-rail-profile"
                    )
                profile_id = args.default_rail_profile
            candidate_rails = trusted_profile_rails_by_environment.get(
                (profile_id, canary_environment),
                set(),
            )
            if profile_id in KNOWN_RAILS:
                matching_rails = candidate_rails & {profile_id}
            else:
                matching_rails = candidate_rails
            if not matching_rails:
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].profile has no matching trust profile "
                    "coverage for canary environment"
                )
            message_type = receipt.get("message_type")
            if (
                isinstance(message_type, str)
                and MESSAGE_TYPE_RE.fullmatch(message_type) is not None
                and not any(
                    message_type in RAIL_MESSAGE_TYPES.get(rail, set())
                    for rail in matching_rails
                )
            ):
                raise EvidenceError(
                    f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                    f"[{receipt_offset}].message_type has no matching trust "
                    "profile rail coverage"
                )


def _require_policy_booleans(args: argparse.Namespace) -> None:
    for attr in (
        "allow_plan_only",
        "allow_dry_run",
        "allow_insecure_http",
        "allow_legacy_colr007",
        "allow_default_profile",
        "allow_failed_receipts",
        "allow_partial_canary",
        "allow_canary_stage_receipts_only",
        "allow_receipt_source_missing",
        "allow_record_only_trust",
        "allow_synthetic_trust",
        "allow_missing_trust_source",
        "allow_profile_json_not_emitted",
    ):
        flag = f"--{attr.replace('_', '-')}"
        setattr(args, attr, _required_cli_bool(getattr(args, attr, None), flag))


def _required_cli_path_sequence(value: Any, label: str) -> list[Path]:
    if value is None:
        return []
    if isinstance(value, (str, bytes)) or type(value) not in (list, tuple):
        raise EvidenceError(f"{label} must be a repeatable path list")
    if len(value) > MAX_EVIDENCE_INPUT_PATHS:
        raise EvidenceError(f"{label} accepts at most {MAX_EVIDENCE_INPUT_PATHS} paths")
    paths: list[Path] = []
    for offset, entry in enumerate(value):
        if isinstance(entry, bytes):
            raise EvidenceError(f"{label}[{offset}] must be a path")
        if isinstance(entry, str):
            paths.append(Path(_plain_text(entry, f"{label}[{offset}]")))
        elif isinstance(entry, Path):
            paths.append(Path(entry))
        else:
            raise EvidenceError(f"{label}[{offset}] must be a path")
    return paths


def run(args: argparse.Namespace) -> int:
    args = _require_plain_namespace(args)
    args.summary_out = _optional_cli_path(getattr(args, "summary_out", None), "summary_out")
    if args.summary_out is not None:
        _reject_output_path_smuggling(args.summary_out, "summary_out")
        _reject_repository_output_path(args.summary_out, "summary_out")
    canary_paths = _required_cli_path_sequence(
        getattr(args, "canary_summary", None),
        "--canary-summary",
    )
    trust_paths = _required_cli_path_sequence(
        getattr(args, "trust_summary", None),
        "--trust-summary",
    )
    receipt_paths = _required_cli_path_sequence(getattr(args, "receipt", None), "--receipt")
    receipt_dir_paths = _required_cli_path_sequence(
        getattr(args, "receipt_dir", None),
        "--receipt-dir",
    )
    args.canary_summary = canary_paths
    args.trust_summary = trust_paths
    args.receipt = receipt_paths
    args.receipt_dir = receipt_dir_paths
    for offset, receipt in enumerate(receipt_paths):
        _reject_repository_artifact_path(receipt, f"receipt[{offset}]")
    for offset, receipt_dir in enumerate(receipt_dir_paths):
        _reject_repository_artifact_path(receipt_dir, f"receipt_dir[{offset}]")
    _require_policy_booleans(args)
    if not canary_paths:
        raise EvidenceError("provide at least one --canary-summary")
    if not trust_paths:
        raise EvidenceError("provide at least one --trust-summary")
    args.provider = _required_cli_string(getattr(args, "provider", None), "--provider")
    args.environment = _required_cli_string(
        getattr(args, "environment", None),
        "--environment",
    )
    args.default_rail_profile = _optional_cli_profile_id(
        getattr(args, "default_rail_profile", None),
        "--default-rail-profile",
    )
    if args.default_rail_profile is not None and not args.allow_default_profile:
        raise EvidenceError("--default-rail-profile requires --allow-default-profile")
    args.max_canary_age_days = _required_positive_cli_int(
        getattr(args, "max_canary_age_days", None),
        "--max-canary-age-days",
    )
    args.max_trust_age_days = _required_positive_cli_int(
        getattr(args, "max_trust_age_days", None),
        "--max-trust-age-days",
    )
    args.max_trust_source_age_days = _required_positive_cli_int(
        getattr(args, "max_trust_source_age_days", None),
        "--max-trust-source-age-days",
    )
    args.receipt_verifier_timeout_secs = _required_positive_finite_cli_number(
        getattr(args, "receipt_verifier_timeout_secs", None),
        "--receipt-verifier-timeout-secs",
    )

    _reject_duplicate_paths(canary_paths, "--canary-summary")
    _reject_duplicate_paths(trust_paths, "--trust-summary")
    _reject_summary_output_input_alias(
        args.summary_out,
        tuple(
            (f"--canary-summary[{offset}]", path)
            for offset, path in enumerate(canary_paths)
        )
        + tuple(
            (f"--trust-summary[{offset}]", path)
            for offset, path in enumerate(trust_paths)
        )
        + tuple(
            (f"--receipt[{offset}]", path)
            for offset, path in enumerate(receipt_paths)
        )
        + tuple(
            (f"--receipt-dir[{offset}]", path)
            for offset, path in enumerate(receipt_dir_paths)
        ),
    )
    _reject_summary_output_receipt_dir_overlap(
        args.summary_out,
        tuple(receipt_dir_paths),
    )
    if args.summary_out is not None:
        _ensure_text_output_target(
            args.summary_out,
            display_label="summary_out",
            create_parent=False,
        )

    canaries = [
        verify_canary_summary(
            path,
            args,
            display_label=f"canary_summaries[{offset}]",
        )
        for offset, path in enumerate(canary_paths)
    ]
    trusts = [
        verify_trust_summary(
            path,
            args,
            display_label=f"trust_summaries[{offset}]",
        )
        for offset, path in enumerate(trust_paths)
    ]
    if args.allow_plan_only and not any(canary["plan_only"] for canary in canaries):
        raise EvidenceError(
            "--allow-plan-only requires at least one canary summary with plan_only=true"
        )
    if args.allow_partial_canary and not any(
        set(canary["stage_names"]) != REQUIRED_CANARY_STAGES for canary in canaries
    ):
        raise EvidenceError(
            "--allow-partial-canary requires at least one canary summary "
            "missing a rail or notary stage"
        )
    if args.allow_profile_json_not_emitted and not any(
        not trust["profile_json_emitted"] for trust in trusts
    ):
        raise EvidenceError(
            "--allow-profile-json-not-emitted requires at least one trust "
            "summary with profile_json_emitted=false"
        )
    if args.allow_dry_run and not any(
        canary["_uses_dry_run_policy"] for canary in canaries
    ):
        raise EvidenceError(
            "--allow-dry-run requires at least one canary stage command or "
            "planned stage with dry_run=true"
        )
    _reject_duplicate_summary_digests(canaries, "canary_summaries")
    _reject_duplicate_summary_digests(trusts, "trust_summaries")
    _reject_compact_json_artifact_path_role_reuse(canaries, trusts)
    _reject_cross_canary_receipt_reuse(canaries)
    _reject_cross_trust_profile_reuse(trusts)
    receipt_summary = verify_receipts(args)
    if receipt_summary is None and not args.allow_canary_stage_receipts_only:
        raise EvidenceError(
            "provide --receipt or --receipt-dir for direct receipt archive verification"
        )
    if receipt_summary is not None:
        if args.allow_canary_stage_receipts_only:
            raise EvidenceError(
                "--allow-canary-stage-receipts-only cannot be combined with "
                "--receipt or --receipt-dir"
            )
        _verify_direct_receipts_cover_canaries(canaries, receipt_summary)
        _reject_compact_json_artifact_path_role_reuse(
            canaries,
            trusts,
            receipt_summary,
        )
    receipt_summaries = _compact_receipt_summaries(canaries, receipt_summary)
    if args.allow_legacy_colr007 and not _receipt_summaries_have_legacy_colr007(
        receipt_summaries
    ):
        raise EvidenceError(
            "--allow-legacy-colr007 requires at least one rail receipt with "
            "legacy colr.007 message_type"
        )
    if args.allow_default_profile and not _receipt_summaries_have_default_profile(
        receipt_summaries
    ):
        raise EvidenceError(
            "--allow-default-profile requires at least one rail receipt without "
            "an explicit profile"
        )
    if args.allow_receipt_source_missing and not _receipt_summaries_have_source_file_gap(
        receipt_summaries
    ):
        raise EvidenceError(
            "--allow-receipt-source-missing requires at least one receipt summary "
            "with require_source_files=false"
        )
    if args.allow_failed_receipts and not (
        any(canary["_uses_failed_receipt_policy"] for canary in canaries)
        or any(summary["allow_failed"] for summary in receipt_summaries)
    ):
        raise EvidenceError(
            "--allow-failed-receipts requires at least one receipt summary "
            "with allow_failed=true"
        )
    if args.allow_insecure_http and not (
        any(canary["_uses_insecure_http_policy"] for canary in canaries)
        or any(summary["allow_insecure_http"] for summary in receipt_summaries)
        or any(trust["allow_insecure_source_url"] for trust in trusts)
    ):
        raise EvidenceError(
            "--allow-insecure-http requires at least one canary command, "
            "receipt summary, or trust summary verified with insecure HTTP"
        )
    if args.allow_record_only_trust and not any(trust["allow_record_only"] for trust in trusts):
        raise EvidenceError(
            "--allow-record-only-trust requires at least one trust summary "
            "verified with allow_record_only=true"
        )
    if args.allow_synthetic_trust and not any(trust["allow_synthetic_der"] for trust in trusts):
        raise EvidenceError(
            "--allow-synthetic-trust requires at least one trust summary "
            "verified with allow_synthetic_der=true"
        )
    if args.allow_missing_trust_source and not _trusts_have_missing_source(trusts):
        raise EvidenceError(
            "--allow-missing-trust-source requires at least one trust profile "
            "with source=null"
        )
    _reject_canary_rail_receipts_without_trust(canaries, trusts, args)
    public_canaries = sorted(
        (_public_canary_summary(canary) for canary in canaries),
        key=_compact_summary_order_key,
    )
    public_trusts = sorted(trusts, key=_compact_summary_order_key)

    output: dict[str, Any] = {
        "version": EVIDENCE_VERSION,
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": True,
        "canary_summaries": public_canaries,
        "trust_summaries": public_trusts,
        "receipt_verification": receipt_summary,
        "policy": {
            "provider": args.provider,
            "environment": args.environment,
            "default_rail_profile": args.default_rail_profile,
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
    text = json.dumps(output, allow_nan=False, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        _write_text_output(args.summary_out, text, display_label="summary_out")
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog=Path(__file__).name,
        description="Verify archived ISO 20022 operator canary and trust evidence.",
        allow_abbrev=False,
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
        "--default-rail-profile",
        help=(
            "Explicit profile id that Torii uses for default-profile rail canaries; "
            "required when --allow-default-profile covers profile=null receipts."
        ),
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
    try:
        normalised_argv = _normalise_cli_argv(argv)
        parser = build_parser()
        _preflight_raw_cli_secrets(
            normalised_argv,
            {
                "--canary-summary",
                "--environment",
                "--max-canary-age-days",
                "--max-trust-age-days",
                "--max-trust-source-age-days",
                "--provider",
                "--receipt",
                "--receipt-dir",
                "--receipt-verifier-timeout-secs",
                "--summary-out",
                "--trust-summary",
                "--default-rail-profile",
            },
        )
        _preflight_boolean_cli_flags(
            normalised_argv,
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
            normalised_argv,
            {"--environment", "--provider"},
            "context",
        )
        _preflight_required_cli_values(
            normalised_argv,
            {"--default-rail-profile"},
            "profile id",
        )
        _preflight_numeric_cli_values(
            normalised_argv,
            integer_flags={
                "--max-canary-age-days",
                "--max-trust-age-days",
                "--max-trust-source-age-days",
            },
            number_flags={"--receipt-verifier-timeout-secs"},
        )
        _preflight_output_cli_paths(
            normalised_argv,
            {
                "--canary-summary",
                "--receipt",
                "--receipt-dir",
                "--summary-out",
                "--trust-summary",
            },
        )
        args = parser.parse_args(normalised_argv)
        return run(args)
    except EvidenceError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
