#!/usr/bin/env python3
"""Verify ISO 20022 operator adapter receipts.

Purpose:
  This CI/operator canary gate validates receipts written by
  ``iso_audit_notary_adapter.py`` and ``iso_rail_gateway_adapter.py``. It
  recomputes each receipt digest, checks success/status policy, verifies HTTPS
  endpoint policy by default, rejects leaked authorization material, and can
  cross-check referenced source XML or notary anchor files.

Prerequisites:
  Python 3.11+. No third party Python packages are required.

Safety:
  The verifier is read-only. It never contacts Torii, rail gateways, or notary
  services and never deletes receipt/source files.
"""

from __future__ import annotations

import argparse
import datetime as dt
import errno
import hashlib
import ipaddress
import json
import os
import re
import stat
import sys
import unicodedata
import urllib.parse
from pathlib import Path
from typing import Any


ANCHOR_DIGEST_FIELD = "anchor_sha256"
ANCHOR_DIR = "anchors"
ANCHOR_VERSION = 1
INDEX_DIGEST_FIELD = "index_sha256"
INDEX_FILE = "messages.index.json"
INDEX_VERSION = 1
LATEST_ANCHOR_FILE = "latest.notary.json"
PERSISTED_RECORD_DIGEST_FIELD = "record_sha256"
PERSISTED_RECORD_VERSION = 1
RECORDS_DIR = "messages"
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
RECEIPT_SUMMARY_VERSION = 2
SUMMARY_DIGEST_FIELD = "summary_sha256"
SUPPORTED_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
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
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.[0-9]{3}$")
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
LOCAL_PATH_ERROR_RE = re.compile(
    r"(?:^|[\s'\"(<:=])(?:[A-Za-z]:[\\/]|/[^/\s'\"<>]+|\.{1,2}/|~/)"
)
MAX_RECEIPT_JSON_BYTES = 4 * 1024 * 1024
MAX_AUDIT_EXPORT_JSON_BYTES = 64 * 1024 * 1024
MAX_PERSISTED_RECORD_JSON_BYTES = 1024 * 1024
MAX_RAIL_XML_BYTES = 4 * 1024 * 1024
MAX_RAIL_SIDECAR_JSON_BYTES = 16 * 1024
MAX_RECEIPT_INPUT_PATHS = 64
MAX_JSON_LIST_ITEMS = 8192
MAX_JSON_OBJECT_MEMBERS = 8192
MAX_JSON_NESTING_DEPTH = 128
MAX_HTTP_URL_CHARS = 2048
MAX_LOCAL_PATH_CHARS = 4096
MAX_CLEAN_STRING_CHARS = 4096
LOCAL_REBINDING_HOST_SUFFIXES = {"localtest.me", "lvh.me", "nip.io", "sslip.io", "vcap.me"}
RESERVED_PLACEHOLDER_HOST_SUFFIXES = {
    "example",
    "example.com",
    "example.invalid",
    "example.net",
    "example.org",
}
TEMPLATE_CANARY_ENDPOINT_HOSTS = {
    "operator-canary.bank",
}
REPOSITORY_XML_FIXTURE_PARTS = (
    "fixtures",
    "iso20022",
)
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
MAX_RAIL_MESSAGE_ID_CHARS = 128
RAIL_MESSAGE_ID_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._:@+-]*[A-Za-z0-9])?$")
SECRET_RESPONSE_PREVIEW_MARKERS = (
    "authorization",
    "bearer ",
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
    "secret",
    "token",
    "x-iroha-signature",
    "x_iroha_signature",
    "x iroha signature",
    "x.iroha.signature",
    "xirohasignature",
)
REDACTED_RESPONSE_PREVIEW = "[redacted: sensitive response body]"
SECRET_VALUE_SCAN_EXEMPT_FIELDS = {"response_body_preview", "error"}
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


def _contains_unsafe_preview_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _contains_non_ascii_preview_text(value: str) -> bool:
    return any(ord(ch) > 0x7E for ch in value)


RAIL_SIDECAR_KEYS = {"message_type", "profile", "payload_sha256", "rail_message_id"}
COMMON_RECEIPT_KEYS = {
    "version",
    "receipt_kind",
    "status_code",
    "ok",
    "response_body_sha256",
    "response_body_preview",
    "error",
    RECEIPT_DIGEST_FIELD,
}
AUDIT_NOTARY_RECEIPT_KEYS = COMMON_RECEIPT_KEYS | {
    "published_at",
    "endpoint",
    "endpoint_sha256",
    "anchor_path",
    ANCHOR_DIGEST_FIELD,
    INDEX_DIGEST_FIELD,
    "record_count",
}
RAIL_GATEWAY_RECEIPT_KEYS = COMMON_RECEIPT_KEYS | {
    "submitted_at",
    "xml_path",
    "sidecar_path",
    "message_type",
    "profile",
    "rail_message_id",
    "payload_sha256",
    "endpoint_url",
    "endpoint_sha256",
}
RECEIPT_KEYS_BY_KIND = {
    "iso-audit-notary": AUDIT_NOTARY_RECEIPT_KEYS,
    "iso-rail-gateway": RAIL_GATEWAY_RECEIPT_KEYS,
}
HTTP_RECEIPT_ERROR_RE = re.compile(r"^HTTP ([1-5][0-9]{2})$")
TRANSPORT_RECEIPT_ERRORS_BY_KIND = {
    "iso-audit-notary": {
        "endpoint transport failed",
        "endpoint transport could not be opened",
        "endpoint response could not be read",
        "endpoint response body was not bytes",
        "endpoint error response could not be read",
        "endpoint error response body was not bytes",
        "invalid HTTP status",
    },
    "iso-rail-gateway": {
        "Torii transport failed",
        "Torii transport could not be opened",
        "Torii response could not be read",
        "Torii response body was not bytes",
        "Torii error response could not be read",
        "Torii error response body was not bytes",
        "invalid HTTP status",
    },
}
AUDIT_INDEX_KEYS = {
    "version",
    "record_count",
    "records",
    INDEX_DIGEST_FIELD,
}
PACS002_CODES = {"ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"}
ISO_RECORD_STATES = {"Pending", "Accepted", "Rejected"}
PERSISTED_RECORD_KEYS = {
    "version",
    "message_id",
    "state",
    "updated_at_ms",
    "transaction_hash",
    "detail",
    "ledger_tx_queued",
    "settled_at_ms",
    "hold_reason_code",
    "change_reason_codes",
    "rejection_reason_code",
    "context",
    "metadata",
    "status_history",
    PERSISTED_RECORD_DIGEST_FIELD,
}
PERSISTED_CONTEXT_KEYS = {
    "ledger_id",
    "source_account_id",
    "source_account_address",
    "target_account_id",
    "target_account_address",
    "asset_definition_id",
    "asset_id",
    "settlement_amount",
    "settlement_currency",
    "settlement_date",
    "settlement_quantity",
    "settlement_movement_type",
    "settlement_payment_type",
    "security_instrument_id",
    "collateral_obligation_id",
    "collateral_original_amount",
    "collateral_original_currency",
    "collateral_original_instrument_id",
    "collateral_substitute_amount",
    "collateral_substitute_currency",
    "collateral_substitute_instrument_id",
    "collateral_effective_date",
    "collateral_substitution_type",
    "collateral_haircut",
    "collateral_reason_code",
    "plan_execution_order",
    "plan_atomicity",
}
PERSISTED_METADATA_KEYS = {
    "profile_id",
    "message_type",
    "business_service",
    "business_message_id",
    "uetr",
    "payload_hash",
    "reference_snapshot_id",
    "embedded_signature_detected",
}
PERSISTED_HISTORY_KEYS = {
    "status",
    "pacs002_code",
    "updated_at_ms",
    "detail",
    "reason_code",
}
AUDIT_INDEX_RECORD_KEYS = {
    "message_id",
    "filename",
    "record_sha256",
    "state",
    "pacs002_code",
    "updated_at_ms",
    "settled_at_ms",
    "transaction_hash",
    "profile_id",
    "message_type",
    "business_message_id",
    "uetr",
    "payload_hash",
    "reference_snapshot_id",
}
ANCHOR_KEYS = {
    "version",
    INDEX_DIGEST_FIELD,
    "record_count",
    "store_dir",
    "audit_index",
    ANCHOR_DIGEST_FIELD,
}


class ReceiptError(RuntimeError):
    """Raised when an operator receipt is invalid."""


def _plain_text(value: str, label: str) -> str:
    try:
        return str.__str__(value)
    except Exception:
        raise ReceiptError(f"{label} must be valid text") from None


def _normalise_cli_argv(argv: list[str] | None) -> list[str]:
    if argv is None:
        raw_sys_argv = sys.argv
        if type(raw_sys_argv) is not list:
            raise ReceiptError("sys.argv must be a plain argument list")
        raw_args = raw_sys_argv[1:]
    else:
        raw_args = argv
    if type(raw_args) is not list:
        raise ReceiptError("argv must be a plain argument list")
    normalised: list[str] = []
    for index, value in enumerate(raw_args):
        if not isinstance(value, str):
            raise ReceiptError(f"argv[{index}] must be a string")
        normalised.append(_plain_text(value, f"argv[{index}]"))
    return normalised


def _require_plain_namespace(args: argparse.Namespace) -> argparse.Namespace:
    if type(args) is not argparse.Namespace:
        raise ReceiptError("args must be an argparse.Namespace")
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


def _path_exists(path: Path, label: str) -> bool:
    try:
        return path.exists()
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot inspect {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot inspect {label}: I/O error") from None


def _path_is_symlink(path: Path, label: str) -> bool:
    try:
        return path.is_symlink()
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot inspect {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot inspect {label}: I/O error") from None


def _path_resolve(path: Path, label: str) -> Path:
    try:
        return path.resolve()
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot resolve {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot resolve {label}: I/O error") from None


def _canonical_summary_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
        allow_nan=False,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")


def _is_lower_hex_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _is_all_zero_sha256(value: str) -> bool:
    return all(ch == "0" for ch in value)


def _reject_all_zero_sha256(value: str, label: str) -> None:
    if _is_all_zero_sha256(value):
        raise ReceiptError(f"{label} must not be all zero")


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
            label = display_label if display_label is not None else str(current)
            raise ReceiptError(
                f"cannot inspect {label} ancestors: {detail}"
            ) from error
        except (RuntimeError, TypeError, ValueError):
            label = display_label if display_label is not None else str(current)
            raise ReceiptError(
                f"cannot inspect {label} ancestors: I/O error"
            ) from None
        if stat.S_ISLNK(mode):
            if path.is_absolute() and current.parent == Path(path.anchor):
                continue
            label = display_label if display_label is not None else str(current)
            raise ReceiptError(f"{label} must not be a symlink")


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise ReceiptError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise ReceiptError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise ReceiptError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise ReceiptError(f"{label} must not contain encoded semicolon parameters")
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise ReceiptError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise ReceiptError(f"{label} must not contain encoded percent characters")
        index += 3


def _reject_raw_cli_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise ReceiptError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise ReceiptError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
        raise ReceiptError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise ReceiptError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise ReceiptError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise ReceiptError(f"{label} must use forward slashes")
    if ";" in raw:
        raise ReceiptError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise ReceiptError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise ReceiptError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise ReceiptError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise ReceiptError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise ReceiptError(f"{label} must not contain dot or parent segments")


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise ReceiptError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if _contains_control_character(arg):
            raise ReceiptError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise ReceiptError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise ReceiptError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise ReceiptError("argument terminator is not supported")
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise ReceiptError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise ReceiptError(f"{arg} does not take a value")
        index += 1


def _preflight_cli_paths(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = _normalise_cli_argv(argv)
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise ReceiptError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise ReceiptError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise ReceiptError(f"{flag} requires a path value")
                _reject_raw_cli_path_smuggling(value, flag)
                _reject_repository_iso_fixture_path(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise ReceiptError(f"{flag} requires a path value")
                _reject_raw_cli_path_smuggling(value, flag)
                _reject_repository_iso_fixture_path(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _read_regular_file(
    path: Path,
    *,
    max_bytes: int | None = None,
    limit_label: str = "input",
    display_label: str | None = None,
) -> bytes:
    label = display_label if display_label is not None else str(path)
    if max_bytes is not None and (
        type(max_bytes) is not int or max_bytes <= 0
    ):
        raise ReceiptError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{label} does not exist") from error
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot inspect {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot inspect {label}: I/O error") from None
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{label} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise ReceiptError(f"{label} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise ReceiptError(f"{label} exceeds {max_bytes} byte {limit_label} limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise ReceiptError(f"{label} must be a regular file")
        if max_bytes is not None and opened.st_size > max_bytes:
            raise ReceiptError(f"{label} exceeds {max_bytes} byte {limit_label} limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise ReceiptError(f"{label} exceeds {max_bytes} byte {limit_label} limit")
        return raw
    except FileNotFoundError as error:
        raise ReceiptError(f"{label} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise ReceiptError(f"{label} must not be a symlink") from error
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot open {label} for reading: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot open {label} for reading: I/O error") from None
    finally:
        if fd >= 0:
            try:
                os.close(fd)
            except (OSError, RuntimeError, TypeError, ValueError):
                pass


def _load_json(
    path: Path,
    *,
    max_bytes: int | None = None,
    display_label: str | None = None,
) -> Any:
    label = display_label if display_label is not None else str(path)
    raw = _read_regular_file(
        path,
        max_bytes=max_bytes,
        limit_label="JSON",
        display_label=display_label,
    )
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_int=_parse_canonical_json_int,
            parse_float=_parse_canonical_json_float,
            parse_constant=_reject_json_constant,
        )
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{label} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise ReceiptError(f"{label} is not valid JSON") from error
    except RecursionError as error:
        raise ReceiptError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(value)
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> set[str]:
    if type(value) is not dict:
        raise ReceiptError(f"{label} contains unknown keys")
    present: set[str] = set()
    for key in value:
        if not isinstance(key, str):
            raise ReceiptError(f"{label} contains unknown keys")
        present.add(_plain_text(key, f"{label} field"))
    if present - allowed:
        raise ReceiptError(f"{label} contains unknown keys")
    return present


def _require_exact_keys(value: dict[str, Any], required: set[str], label: str) -> None:
    present = _reject_unknown_keys(value, required, label)
    missing = sorted(required - present)
    if missing:
        raise ReceiptError(f"{label} is missing required keys: {', '.join(missing)}")


def _require_json_array(value: Any, label: str) -> list[Any]:
    if type(value) is not list:
        raise ReceiptError(f"{label} must be an array")
    if len(value) > MAX_JSON_LIST_ITEMS:
        raise ReceiptError(f"{label} must contain at most {MAX_JSON_LIST_ITEMS} items")
    return value


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


def _contains_control_character(value: str) -> bool:
    return any(
        ord(ch) < 0x20 or ord(ch) == 0x7F or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _contains_unsafe_json_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\r", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise ReceiptError(f"{label} must not contain secret-looking material")


def _reject_non_ascii_identifier(value: str, label: str) -> None:
    if any(ord(ch) > 0x7E for ch in value):
        raise ReceiptError(f"{label} must use printable ASCII")


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    if len(pairs) > MAX_JSON_OBJECT_MEMBERS:
        raise ReceiptError(
            f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
        )
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise ReceiptError("JSON object contains duplicate key")
        seen.add(key)
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise ReceiptError("JSON contains non-finite numeric constant")


def _parse_canonical_json_int(value: str) -> int:
    if JSON_CANONICAL_INT_RE.fullmatch(value) is None:
        raise ReceiptError("JSON contains non-canonical numeric value")
    return int(value, 10)


def _parse_canonical_json_float(value: str) -> float:
    if CLI_CANONICAL_NUMBER_RE.fullmatch(value) is None:
        raise ReceiptError("JSON contains non-canonical numeric value")
    parsed = float(value)
    if parsed == float("inf") or parsed == float("-inf"):
        raise ReceiptError("JSON contains non-finite numeric constant")
    if parsed == 0.0 and value.startswith("-"):
        raise ReceiptError("JSON contains non-canonical numeric value")
    return parsed


def _reject_json_surrogates(value: Any, *, _depth: int = 0) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise ReceiptError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, str):
        value = _plain_text(value, "JSON string")
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise ReceiptError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        if type(value) is not list:
            raise ReceiptError("JSON array must be a plain array")
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise ReceiptError(
                f"JSON array must contain at most {MAX_JSON_LIST_ITEMS} items"
            )
        for item in value:
            _reject_json_surrogates(item, _depth=_depth + 1)
    elif isinstance(value, dict):
        if type(value) is not dict:
            raise ReceiptError("JSON object must be a plain object")
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise ReceiptError(
                f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
            )
        for key, item in value.items():
            _reject_json_surrogates(key, _depth=_depth + 1)
            _reject_json_surrogates(item, _depth=_depth + 1)


def digest_without_field(obj: dict[str, Any], digest_field: str) -> str:
    """Compute the canonical object digest with one digest field removed."""

    if digest_field not in obj:
        raise ReceiptError(f"missing {digest_field}")
    body = dict(obj)
    body.pop(digest_field)
    return sha256_hex(_canonical_json_bytes(body))


def require_digest_matches(obj: dict[str, Any], digest_field: str, label: str) -> str:
    """Validate and return an embedded digest field."""

    expected = obj.get(digest_field)
    if not _is_lower_hex_sha256(expected):
        raise ReceiptError(f"{label} has missing or non-canonical {digest_field}")
    _reject_all_zero_sha256(expected, f"{label}.{digest_field}")
    actual = digest_without_field(obj, digest_field)
    if actual != expected:
        raise ReceiptError(f"{label} {digest_field} mismatch")
    return expected


def _check_no_secret_material(
    value: Any,
    path: Path,
    *,
    field_name: str | None = None,
    _depth: int = 0,
) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise ReceiptError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, dict):
        if type(value) is not dict:
            raise ReceiptError(f"{path} contains non-plain JSON object")
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise ReceiptError(
                f"{path} must contain at most {MAX_JSON_OBJECT_MEMBERS} object members"
            )
        for key, child in value.items():
            if not isinstance(key, str):
                raise ReceiptError(f"{path} contains forbidden non-string field")
            key_text = _plain_text(key, f"{path} field")
            if _is_secret_looking_key(key_text):
                raise ReceiptError(f"{path} contains forbidden secret-looking field")
            if _is_control_bearing_key(key_text):
                raise ReceiptError(f"{path} contains forbidden control-bearing field")
            _check_no_secret_material(
                child,
                path,
                field_name=key_text,
                _depth=_depth + 1,
            )
    elif isinstance(value, list):
        if type(value) is not list:
            raise ReceiptError(f"{path} contains non-plain JSON array")
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise ReceiptError(f"{path} must contain at most {MAX_JSON_LIST_ITEMS} items")
        for child in value:
            _check_no_secret_material(child, path, _depth=_depth + 1)
    elif isinstance(value, str):
        value = _plain_text(value, str(path))
        if _contains_unsafe_json_control(value):
            raise ReceiptError(f"{path} contains unsafe control characters")
        if field_name not in SECRET_VALUE_SCAN_EXEMPT_FIELDS and _contains_secret_material(value):
            raise ReceiptError(f"{path} contains secret-looking material")


def _reject_url_control_chars(url: str, label: str) -> None:
    if _contains_control_character(url):
        raise ReceiptError(f"{label} must not contain control characters")


def _reject_url_percent_encoding_smuggling(url: str, label: str) -> None:
    index = 0
    while True:
        index = url.find("%", index)
        if index == -1:
            return
        token = url[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise ReceiptError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise ReceiptError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        index += 3


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


def _validate_url_host(parsed: urllib.parse.ParseResult, label: str) -> None:
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise ReceiptError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise ReceiptError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise ReceiptError(f"{label} host must not end with a dot")
    if any(ord(ch) > 0x7E for ch in raw_host):
        raise ReceiptError(f"{label} host must use printable ASCII")
    _reject_secret_looking_identifier(raw_host, f"{label} host")
    if len(raw_host) > 253:
        raise ReceiptError(f"{label} host must be at most 253 characters")
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise ReceiptError(f"{label} host must be a valid IP address")
    _reject_legacy_ipv4_host_notation(raw_host, label)
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise ReceiptError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise ReceiptError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise ReceiptError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise ReceiptError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise ReceiptError(
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
        raise ReceiptError(f"{label} host must not use legacy IPv4 numeric notation")


def _reject_local_url_host(
    parsed: urllib.parse.ParseResult,
    label: str,
    *,
    allow_insecure_http: bool,
) -> None:
    if allow_insecure_http:
        return
    hostname = (parsed.hostname or "").strip().lower()
    if hostname == "localhost" or hostname.endswith(".localhost"):
        raise ReceiptError(f"{label} must not use localhost")
    if _host_uses_rebinding_suffix(hostname):
        raise ReceiptError(f"{label} must not use local/private rebinding hostnames")
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        return
    if not address.is_global:
        raise ReceiptError(f"{label} must not use local, private, or reserved IP addresses")
    if _address_embeds_non_global_ipv4(address):
        raise ReceiptError(f"{label} must not embed local, private, or reserved IPv4 addresses")


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


def _host_uses_rebinding_suffix(hostname: str) -> bool:
    return hostname in LOCAL_REBINDING_HOST_SUFFIXES or any(
        hostname.endswith("." + suffix) for suffix in LOCAL_REBINDING_HOST_SUFFIXES
    )


def _host_uses_reserved_placeholder_suffix(hostname: str) -> bool:
    return hostname in RESERVED_PLACEHOLDER_HOST_SUFFIXES or any(
        hostname.endswith("." + suffix) for suffix in RESERVED_PLACEHOLDER_HOST_SUFFIXES
    )


def _host_uses_template_canary_suffix(hostname: str) -> bool:
    return hostname in TEMPLATE_CANARY_ENDPOINT_HOSTS or any(
        hostname.endswith("." + suffix) for suffix in TEMPLATE_CANARY_ENDPOINT_HOSTS
    )


def _reject_reserved_placeholder_url_host(
    parsed: urllib.parse.ParseResult,
    label: str,
) -> None:
    hostname = (parsed.hostname or "").strip().lower()
    if _host_uses_reserved_placeholder_suffix(hostname):
        raise ReceiptError(f"{label} must not use reserved placeholder hostnames")


def _reject_template_canary_url_host(
    parsed: urllib.parse.ParseResult,
    label: str,
) -> None:
    hostname = (parsed.hostname or "").strip().lower()
    if _host_uses_template_canary_suffix(hostname):
        raise ReceiptError(f"{label} must not use template canary hostnames")


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


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if any(ord(ch) > 0x7E for ch in path):
        raise ReceiptError(f"{label} path must use printable ASCII")
    if "\\" in path:
        raise ReceiptError(f"{label} path must use forward slashes")
    if ";" in path:
        raise ReceiptError(f"{label} path must not contain semicolon parameters")
    if any(token in path for token in (":", "@", "[", "]")):
        raise ReceiptError(f"{label} path must not contain URL delimiter characters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise ReceiptError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise ReceiptError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
        raise ReceiptError(f"{label} path must not contain secret-looking material")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise ReceiptError(f"{label} path must not contain encoded dot or separator characters")
    if "%3b" in lowered:
        raise ReceiptError(f"{label} path must not contain encoded semicolon parameters")
    if any(token in lowered for token in ("%23", "%3a", "%3f", "%40", "%5b", "%5d")):
        raise ReceiptError(f"{label} path must not contain encoded URL delimiter characters")
    if "%25" in lowered:
        raise ReceiptError(f"{label} path must not contain encoded percent characters")
    if re.search(r"%[89a-f][0-9a-f]", lowered):
        raise ReceiptError(f"{label} path must not contain percent-encoded non-ASCII bytes")


def _require_clean_string(value: Any, label: str) -> str:
    if not isinstance(value, str):
        raise ReceiptError(f"{label} must be a non-empty string")
    value = _plain_text(value, label)
    if not value.strip():
        raise ReceiptError(f"{label} must be a non-empty string")
    if len(value) > MAX_CLEAN_STRING_CHARS:
        raise ReceiptError(f"{label} must be no longer than {MAX_CLEAN_STRING_CHARS} characters")
    if _contains_control_character(value):
        raise ReceiptError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    return value


def _require_nonsecret_clean_string(value: Any, label: str) -> str:
    text = _require_clean_string(value, label)
    if _contains_secret_material(text) or _contains_secret_identifier_material(text):
        raise ReceiptError(f"{label} must not contain secret-looking material")
    return text


def _require_optional_clean_string(value: Any, label: str) -> str | None:
    if value is None:
        return None
    return _require_clean_string(value, label)


def _require_optional_nonsecret_clean_string(value: Any, label: str) -> str | None:
    if value is None:
        return None
    return _require_nonsecret_clean_string(value, label)


def _require_clean_path_string(value: Any, label: str) -> str:
    path = _require_clean_string(value, label)
    if len(path) > MAX_LOCAL_PATH_CHARS:
        raise ReceiptError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if any(ch.isspace() for ch in path):
        raise ReceiptError(f"{label} must not contain whitespace")
    if path.startswith("-"):
        raise ReceiptError(f"{label} must not start with a dash")
    if "\\" in path:
        raise ReceiptError(f"{label} must use forward slashes")
    if ";" in path:
        raise ReceiptError(f"{label} must not contain semicolon path parameters")
    if ":" in path:
        raise ReceiptError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
        raise ReceiptError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(path, label)
    parts = path.split("/")
    checked_parts = parts[1:] if path.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise ReceiptError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise ReceiptError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise ReceiptError(f"{label} must not contain dot or parent segments")
    return path


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


def _path_is_repository_iso_fixture(raw: str) -> bool:
    return _path_contains_component_sequence(raw, REPOSITORY_XML_FIXTURE_PARTS)


def _reject_repository_iso_fixture_path(raw: str | Path, label: str) -> None:
    if _path_is_repository_iso_fixture(str(raw)):
        raise ReceiptError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )


def _require_nonnegative_int(value: Any, label: str) -> int:
    if type(value) is not int or value < 0:
        raise ReceiptError(f"{label} must be a non-negative integer")
    return value


def _require_bool(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise ReceiptError(f"{label} must be a boolean")
    return value


def _require_optional_nonnegative_int(value: Any, label: str) -> int | None:
    if value is None:
        return None
    return _require_nonnegative_int(value, label)


def _expected_message_filename(message_id: str) -> str:
    return f"{sha256_hex(message_id.encode('utf-8'))}.json"


def _require_pacs002_code(value: Any, label: str) -> str:
    code = _require_clean_string(value, label)
    if code not in PACS002_CODES:
        raise ReceiptError(f"{label} must be a supported pacs.002 status code")
    return code


def _require_record_state(value: Any, label: str) -> str:
    state = _require_clean_string(value, label)
    if state not in ISO_RECORD_STATES:
        raise ReceiptError(f"{label} must be Pending, Accepted, or Rejected")
    return state


def _require_status_code_consistency(
    state: str,
    code: str,
    label: str,
    *,
    noun: str,
) -> None:
    allowed = {
        "Pending": {"ACTC", "ACSP", "ACWC", "PDNG"},
        "Accepted": {"ACSP", "ACSC"},
        "Rejected": {"RJCT"},
    }[state]
    if code not in allowed:
        raise ReceiptError(
            f"{label}.pacs002_code is not valid for {state} {noun}"
        )


def _require_audit_record_status_consistency(record: dict[str, Any], label: str) -> None:
    state = _require_record_state(record.get("state"), f"{label}.state")
    code = _require_pacs002_code(record.get("pacs002_code"), f"{label}.pacs002_code")
    _require_status_code_consistency(state, code, label, noun="state")


def _derived_pacs002_code(record: dict[str, Any], label: str) -> str:
    state = _require_record_state(record.get("state"), f"{label}.state")
    if state == "Rejected":
        return "RJCT"
    if state == "Accepted":
        return "ACSC" if record.get("settled_at_ms") is not None else "ACSP"
    if record.get("hold_reason_code") is not None:
        return "PDNG"
    change_reason_codes = record.get("change_reason_codes")
    if type(change_reason_codes) is list and len(change_reason_codes) > 0:
        return "ACWC"
    if record.get("ledger_tx_queued"):
        return "ACSP"
    return "ACTC"


def _verify_optional_clean_string_fields(
    value: dict[str, Any], keys: set[str], label: str
) -> None:
    for key in keys:
        _require_optional_nonsecret_clean_string(value.get(key), f"{label}.{key}")


def _verify_persisted_context(value: Any, label: str) -> None:
    if type(value) is not dict:
        raise ReceiptError(f"{label} must be an object")
    _require_exact_keys(value, PERSISTED_CONTEXT_KEYS, label)
    _verify_optional_clean_string_fields(value, PERSISTED_CONTEXT_KEYS, label)


def _verify_persisted_metadata(
    value: Any, label: str, index_record: dict[str, Any]
) -> None:
    if type(value) is not dict:
        raise ReceiptError(f"{label} must be an object")
    _require_exact_keys(value, PERSISTED_METADATA_KEYS, label)
    _verify_optional_clean_string_fields(
        value,
        PERSISTED_METADATA_KEYS - {"embedded_signature_detected"},
        label,
    )
    _require_bool(
        value.get("embedded_signature_detected"),
        f"{label}.embedded_signature_detected",
    )
    payload_hash = value.get("payload_hash")
    if payload_hash is not None and not _is_lower_hex_sha256(payload_hash):
        raise ReceiptError(f"{label}.payload_hash must be a canonical SHA-256")
    if payload_hash is not None:
        _reject_all_zero_sha256(payload_hash, f"{label}.payload_hash")
    for key in (
        "profile_id",
        "message_type",
        "business_message_id",
        "uetr",
        "payload_hash",
        "reference_snapshot_id",
    ):
        if value.get(key) != index_record.get(key):
            raise ReceiptError(f"{label}.{key} does not match audit index record")


def _verify_persisted_history_entry(value: Any, label: str) -> tuple[str, str, int]:
    if type(value) is not dict:
        raise ReceiptError(f"{label} must be an object")
    _require_exact_keys(value, PERSISTED_HISTORY_KEYS, label)
    status = _require_record_state(value.get("status"), f"{label}.status")
    code = _require_pacs002_code(value.get("pacs002_code"), f"{label}.pacs002_code")
    _require_status_code_consistency(status, code, label, noun="status")
    updated_at_ms = _require_nonnegative_int(
        value.get("updated_at_ms"),
        f"{label}.updated_at_ms",
    )
    _require_optional_nonsecret_clean_string(value.get("detail"), f"{label}.detail")
    _require_optional_nonsecret_clean_string(value.get("reason_code"), f"{label}.reason_code")
    return status, code, updated_at_ms


def _verify_persisted_record_source(
    index_record: dict[str, Any],
    path: Path,
    label: str,
) -> None:
    value = _load_json(
        path,
        max_bytes=MAX_PERSISTED_RECORD_JSON_BYTES,
        display_label=label,
    )
    if type(value) is not dict:
        raise ReceiptError(f"{label} must contain a JSON object")
    _require_exact_keys(value, PERSISTED_RECORD_KEYS, label)
    version = value.get("version")
    if type(version) is not int or version != PERSISTED_RECORD_VERSION:
        raise ReceiptError(f"{label} has unsupported persisted record version")
    source_digest = require_digest_matches(value, PERSISTED_RECORD_DIGEST_FIELD, label)
    if source_digest != index_record.get(PERSISTED_RECORD_DIGEST_FIELD):
        raise ReceiptError(f"{label} record_sha256 does not match audit index record")
    if value.get("message_id") != index_record.get("message_id"):
        raise ReceiptError(f"{label}.message_id does not match audit index record")
    if value.get("state") != index_record.get("state"):
        raise ReceiptError(f"{label}.state does not match audit index record")
    updated_at_ms = _require_nonnegative_int(
        value.get("updated_at_ms"),
        f"{label}.updated_at_ms",
    )
    if updated_at_ms != index_record.get("updated_at_ms"):
        raise ReceiptError(f"{label}.updated_at_ms does not match audit index record")
    _require_optional_nonnegative_int(value.get("settled_at_ms"), f"{label}.settled_at_ms")
    if value.get("settled_at_ms") != index_record.get("settled_at_ms"):
        raise ReceiptError(f"{label}.settled_at_ms does not match audit index record")
    for key in (
        "transaction_hash",
        "detail",
        "hold_reason_code",
        "rejection_reason_code",
    ):
        _require_optional_nonsecret_clean_string(value.get(key), f"{label}.{key}")
    if value.get("transaction_hash") != index_record.get("transaction_hash"):
        raise ReceiptError(f"{label}.transaction_hash does not match audit index record")
    _require_bool(value.get("ledger_tx_queued"), f"{label}.ledger_tx_queued")
    change_reason_codes = _require_json_array(
        value.get("change_reason_codes"),
        f"{label}.change_reason_codes",
    )
    for offset, code in enumerate(change_reason_codes):
        _require_nonsecret_clean_string(code, f"{label}.change_reason_codes[{offset}]")
    _verify_persisted_context(value.get("context"), f"{label}.context")
    _verify_persisted_metadata(
        value.get("metadata"),
        f"{label}.metadata",
        index_record,
    )
    history = _require_json_array(value.get("status_history"), f"{label}.status_history")
    if not history:
        raise ReceiptError(f"{label}.status_history must be a non-empty array")
    last_status = None
    last_code = None
    last_updated_at_ms = None
    previous_updated_at_ms = None
    for offset, entry in enumerate(history):
        last_status, last_code, last_updated_at_ms = _verify_persisted_history_entry(
            entry,
            f"{label}.status_history[{offset}]",
        )
        if (
            previous_updated_at_ms is not None
            and last_updated_at_ms < previous_updated_at_ms
        ):
            raise ReceiptError(
                f"{label}.status_history[{offset}].updated_at_ms must not move backwards"
            )
        previous_updated_at_ms = last_updated_at_ms
    derived_code = _derived_pacs002_code(value, label)
    if derived_code != index_record.get("pacs002_code"):
        raise ReceiptError(f"{label}.pacs002_code does not match persisted state")
    if last_status != value.get("state") or last_code != derived_code:
        raise ReceiptError(f"{label}.status_history does not end with current status")
    if last_updated_at_ms != updated_at_ms:
        raise ReceiptError(f"{label}.status_history does not end at current updated_at_ms")


def _require_https(url: str, *, allow_insecure_http: bool, label: str) -> None:
    url_label = f"{label} URL"
    if len(url) > MAX_HTTP_URL_CHARS:
        raise ReceiptError(f"{url_label} must be no longer than {MAX_HTTP_URL_CHARS} characters")
    _reject_url_control_chars(url, label)
    _reject_url_percent_encoding_smuggling(url, label)
    if url != url.strip():
        raise ReceiptError(f"{url_label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in url):
        raise ReceiptError(f"{url_label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise ReceiptError(f"{url_label} is not valid") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise ReceiptError(f"{label} uses insecure HTTP URL")
        raise ReceiptError(f"{url_label} must use http or https")
    try:
        port = parsed.port
    except ValueError as error:
        raise ReceiptError(f"{url_label} has invalid port") from error
    port_text = _raw_url_port_text(parsed)
    if port_text == "":
        raise ReceiptError(f"{url_label} must not include an empty port")
    if port_text is not None:
        if len(port_text) > 1 and port_text.startswith("0"):
            raise ReceiptError(f"{url_label} port must not contain leading zeros")
        if port == 0:
            raise ReceiptError(f"{url_label} port must be positive")
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise ReceiptError(f"{url_label} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise ReceiptError(f"{url_label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise ReceiptError(f"{url_label} must not contain credentials")
    _validate_url_host(parsed, url_label)
    _reject_local_url_host(
        parsed,
        url_label,
        allow_insecure_http=allow_insecure_http,
    )
    if parsed.params or parsed.query or parsed.fragment:
        raise ReceiptError(f"{url_label} must not contain params, query, or fragment")
    _validate_url_path(parsed, url_label)
    _reject_reserved_placeholder_url_host(parsed, url_label)
    _reject_template_canary_url_host(parsed, url_label)


def _check_status(receipt: dict[str, Any], label: str, *, allow_failed: bool) -> None:
    ok = receipt.get("ok")
    status_code = receipt.get("status_code")
    if not isinstance(ok, bool):
        raise ReceiptError(f"{label} ok must be boolean")
    if status_code is not None and (
        type(status_code) is not int
        or status_code < 100
        or status_code > 599
    ):
        raise ReceiptError(f"{label} status_code must be null or an HTTP status integer")
    success = type(status_code) is int and 200 <= status_code <= 299
    if ok != success:
        raise ReceiptError(f"{label} ok does not match status_code success state")
    if not allow_failed and not success:
        raise ReceiptError(f"{label} is not a successful 2xx receipt")


def _check_timestamp(receipt: dict[str, Any], key: str, label: str) -> None:
    value = _require_clean_string(receipt.get(key), f"{label} {key}")
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise ReceiptError(f"{label} {key} is not a valid ISO timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ReceiptError(f"{label} {key} must include a timezone offset")
    if CANONICAL_TIMESTAMP_RE.fullmatch(value) is None or value.endswith("-00:00"):
        raise ReceiptError(f"{label} {key} must use a canonical ISO timestamp")


def _check_endpoint_digest(receipt: dict[str, Any], label: str, endpoint: str) -> None:
    endpoint_sha256 = receipt.get("endpoint_sha256")
    if not _is_lower_hex_sha256(endpoint_sha256):
        raise ReceiptError(f"{label} has invalid endpoint_sha256")
    actual = sha256_hex(endpoint.encode("utf-8"))
    if endpoint_sha256 != actual:
        raise ReceiptError(f"{label} endpoint_sha256 does not match endpoint")


def _check_failed_receipt_error(
    receipt: dict[str, Any],
    label: str,
    *,
    kind: str,
    error: str,
) -> None:
    status_code = receipt.get("status_code")
    if type(status_code) is int:
        match = HTTP_RECEIPT_ERROR_RE.fullmatch(error)
        if match is None or int(match.group(1)) != status_code:
            raise ReceiptError(f"{label} failed receipt error must match status_code")
        return
    if error not in TRANSPORT_RECEIPT_ERRORS_BY_KIND.get(kind, set()):
        raise ReceiptError(f"{label} failed receipt error is unsupported")


def _check_response_metadata(receipt: dict[str, Any], label: str, *, kind: str) -> None:
    status_code = receipt.get("status_code")
    has_http_response = type(status_code) is int
    response_body_sha256 = receipt.get("response_body_sha256")
    response_body_preview = receipt.get("response_body_preview")
    if response_body_sha256 is None:
        if has_http_response:
            raise ReceiptError(f"{label} response_body_sha256 must be recorded for HTTP response")
        if response_body_preview is not None:
            raise ReceiptError(f"{label} response_body_preview requires response_body_sha256")
    else:
        if not has_http_response:
            raise ReceiptError(f"{label} response_body_sha256 requires HTTP status_code")
        if not _is_lower_hex_sha256(response_body_sha256):
            raise ReceiptError(f"{label} has invalid response_body_sha256")
        _reject_all_zero_sha256(response_body_sha256, f"{label} response_body_sha256")
        if not isinstance(response_body_preview, str):
            raise ReceiptError(f"{label} response_body_preview must be a string")
        if len(response_body_preview) > 4096:
            raise ReceiptError(f"{label} response_body_preview exceeds 4096 characters")
        if _contains_unsafe_preview_control(response_body_preview):
            raise ReceiptError(
                f"{label} response_body_preview contains unsafe control characters"
            )
        if _contains_non_ascii_preview_text(response_body_preview):
            raise ReceiptError(f"{label} response_body_preview contains non-ASCII text")
        if _response_preview_looks_secret(response_body_preview):
            raise ReceiptError(f"{label} response_body_preview contains secret-looking material")
        if "\n" in response_body_preview or "\t" in response_body_preview:
            raise ReceiptError(f"{label} response_body_preview must be single-line text")
        if receipt.get("ok") and response_body_preview == REDACTED_RESPONSE_PREVIEW:
            raise ReceiptError(
                f"{label} successful receipt must not carry redacted response_body_preview"
            )

    error = receipt.get("error")
    if error is not None:
        error = _normalize_optional_string(error, f"{label} error")
        if _contains_unsafe_preview_control(error):
            raise ReceiptError(f"{label} error contains unsafe control characters")
        if _contains_non_ascii_preview_text(error):
            raise ReceiptError(f"{label} error contains non-ASCII text")
        if _response_preview_looks_secret(error):
            raise ReceiptError(f"{label} error contains secret-looking material")
        if _contains_local_path_material(error):
            raise ReceiptError(f"{label} error contains local path material")
    if receipt.get("ok") and error is not None:
        raise ReceiptError(f"{label} successful receipt must not record error")
    if receipt.get("ok") is False and error is None:
        raise ReceiptError(f"{label} failed receipt must record error")
    if receipt.get("ok") is False and error is not None:
        _check_failed_receipt_error(receipt, label, kind=kind, error=error)


def _contains_local_path_material(value: str) -> bool:
    return (
        "\\" in value
        or "file:" in value.casefold()
        or LOCAL_PATH_ERROR_RE.search(value) is not None
    )


def _response_preview_looks_secret(preview: str) -> bool:
    return _contains_secret_material(preview) or any(
        _contains_secret_marker(candidate, SECRET_RESPONSE_PREVIEW_MARKERS)
        for candidate in _secret_scan_values(preview)
    )


def _verify_audit_index_record_source(record: Any, label: str) -> None:
    if type(record) is not dict:
        raise ReceiptError(f"{label} must be an object")
    _require_exact_keys(record, AUDIT_INDEX_RECORD_KEYS, label)
    message_id = _require_nonsecret_clean_string(
        record.get("message_id"),
        f"{label}.message_id",
    )
    filename = _require_nonsecret_clean_string(record.get("filename"), f"{label}.filename")
    expected_filename = _expected_message_filename(message_id)
    if filename != expected_filename:
        raise ReceiptError(
            f"{label}.filename must be digest-addressed as {expected_filename}"
        )
    if not _is_lower_hex_sha256(record.get("record_sha256")):
        raise ReceiptError(f"{label}.record_sha256 must be a canonical SHA-256")
    _reject_all_zero_sha256(record["record_sha256"], f"{label}.record_sha256")
    _require_audit_record_status_consistency(record, label)
    _require_nonnegative_int(record.get("updated_at_ms"), f"{label}.updated_at_ms")
    _require_optional_nonnegative_int(record.get("settled_at_ms"), f"{label}.settled_at_ms")
    _require_optional_nonsecret_clean_string(
        record.get("transaction_hash"),
        f"{label}.transaction_hash",
    )
    _require_optional_nonsecret_clean_string(record.get("profile_id"), f"{label}.profile_id")
    _require_optional_nonsecret_clean_string(
        record.get("message_type"),
        f"{label}.message_type",
    )
    _require_optional_nonsecret_clean_string(
        record.get("business_message_id"),
        f"{label}.business_message_id",
    )
    _require_optional_nonsecret_clean_string(record.get("uetr"), f"{label}.uetr")
    payload_hash = _require_optional_nonsecret_clean_string(
        record.get("payload_hash"),
        f"{label}.payload_hash",
    )
    if payload_hash is not None and not _is_lower_hex_sha256(payload_hash):
        raise ReceiptError(f"{label}.payload_hash must be a canonical SHA-256")
    if payload_hash is not None:
        _reject_all_zero_sha256(payload_hash, f"{label}.payload_hash")
    _require_optional_nonsecret_clean_string(
        record.get("reference_snapshot_id"),
        f"{label}.reference_snapshot_id",
    )


def _reject_duplicate_audit_index_records(records: list[Any], label: str) -> None:
    seen: dict[str, dict[str, int]] = {
        "message_id": {},
        "filename": {},
        PERSISTED_RECORD_DIGEST_FIELD: {},
    }
    for offset, record in enumerate(records):
        for field, field_seen in seen.items():
            value = record[field]
            if value in field_seen:
                raise ReceiptError(
                    f"{label}.records[{offset}].{field} duplicates "
                    f"{label}.records[{field_seen[value]}].{field}"
                )
            field_seen[value] = offset


def _verify_audit_index_source(value: Any, label: str) -> dict[str, Any]:
    if type(value) is not dict:
        raise ReceiptError(f"{label} must contain a JSON object")
    _reject_unknown_keys(value, AUDIT_INDEX_KEYS, label)
    version = value.get("version")
    if type(version) is not int or version != INDEX_VERSION:
        raise ReceiptError(f"{label} version must be {INDEX_VERSION}")
    require_digest_matches(value, INDEX_DIGEST_FIELD, label)
    record_count = value.get("record_count")
    records = value.get("records")
    if type(record_count) is not int or record_count < 0:
        raise ReceiptError(f"{label} record_count must be a non-negative integer")
    records = _require_json_array(records, f"{label} records")
    if len(records) != record_count:
        raise ReceiptError(f"{label} record_count does not match records length")
    for offset, record in enumerate(records):
        _verify_audit_index_record_source(record, f"{label}.records[{offset}]")
    _reject_duplicate_audit_index_records(records, label)
    return value


def _ensure_input_directory(
    path: Path,
    label: str,
    *,
    display_path: bool = True,
) -> None:
    display = f"{label} {path}" if display_path else label
    _reject_symlinked_existing_ancestors(path.parent, display_label=display)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{display} does not exist") from error
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot inspect {display}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot inspect {display}: I/O error") from None
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{display} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise ReceiptError(f"{display} must be a directory")


def _record_store_dir(anchor: dict[str, Any], label: str) -> Path | None:
    store_dir = anchor.get("store_dir")
    if store_dir is None:
        return None
    return Path(_require_clean_path_string(store_dir, f"{label}.store_dir"))


def _verify_persisted_record_sources(
    audit_index: dict[str, Any],
    store_dir: Path | None,
    label: str,
    *,
    require_source_files: bool,
) -> None:
    records = _require_json_array(audit_index.get("records"), f"{label}.records")
    if store_dir is None:
        if require_source_files and records:
            raise ReceiptError(f"{label}.store_dir is required to verify audit records")
        return
    if not _path_exists(store_dir, f"{label}.store_dir"):
        if require_source_files:
            raise ReceiptError(f"{label}.store_dir does not exist")
        return
    _ensure_input_directory(store_dir, f"{label}.store_dir", display_path=False)
    messages_dir = store_dir / RECORDS_DIR
    if not _path_exists(messages_dir, f"{label}.store_dir/{RECORDS_DIR}"):
        if require_source_files:
            raise ReceiptError(f"{label}.store_dir/{RECORDS_DIR} does not exist")
        return
    _ensure_input_directory(
        messages_dir,
        f"{label}.store_dir/{RECORDS_DIR}",
        display_path=False,
    )
    for offset, record in enumerate(records):
        if type(record) is not dict:
            raise ReceiptError(f"{label}.records[{offset}] must be an object")
        record_path = messages_dir / record["filename"]
        if not _path_exists(record_path, f"{label}.records[{offset}].source"):
            if require_source_files:
                raise ReceiptError(f"{label} references missing audit record")
            continue
        _verify_persisted_record_source(
            record,
            record_path,
            f"{label}.records[{offset}].source",
        )


def _anchor_export_dir_for_convention(
    anchor_path: Path,
    *,
    index_sha256: str,
    receipt_label: str,
) -> Path:
    if anchor_path.name == LATEST_ANCHOR_FILE:
        return anchor_path.parent
    if anchor_path.parent.name == ANCHOR_DIR:
        expected_name = f"{index_sha256}.notary.json"
        if anchor_path.name != expected_name:
            raise ReceiptError(
                f"{receipt_label} anchor_path filename must be digest-addressed as {expected_name}"
            )
        return anchor_path.parent.parent
    raise ReceiptError(
        f"{receipt_label} anchor_path must be {LATEST_ANCHOR_FILE} or {ANCHOR_DIR}/<index_sha256>.notary.json"
    )


def _verify_anchor_path_peers(
    anchor_path: Path,
    export_dir: Path,
    *,
    index_sha256: str,
    receipt_label: str,
    require_source_files: bool,
) -> None:
    if anchor_path.name != LATEST_ANCHOR_FILE:
        return
    digest_anchor = export_dir / ANCHOR_DIR / f"{index_sha256}.notary.json"
    digest_anchor_label = f"{receipt_label} digest-addressed anchor peer"
    if _path_is_symlink(digest_anchor, digest_anchor_label):
        raise ReceiptError(
            f"{receipt_label} digest-addressed anchor peer must not be a symlink"
        )
    if _path_exists(digest_anchor, digest_anchor_label):
        if _read_regular_file(
            digest_anchor,
            max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES,
            limit_label="JSON",
            display_label=digest_anchor_label,
        ) != _read_regular_file(
            anchor_path,
            max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES,
            limit_label="JSON",
            display_label=f"{receipt_label} latest anchor",
        ):
            raise ReceiptError(
                f"{receipt_label} latest anchor differs from digest-addressed peer"
            )
    elif require_source_files:
        raise ReceiptError(f"{receipt_label} latest anchor has no digest-addressed peer")


def _verify_anchor_source(
    receipt: dict[str, Any],
    path: Path,
    *,
    require_source_files: bool,
    display_label: str | None = None,
) -> tuple[str | None, str | None]:
    label = display_label or str(path)
    anchor_label = f"{label} anchor source"
    exported_index_label = f"{label} exported audit index"
    anchor_sha256 = receipt.get(ANCHOR_DIGEST_FIELD)
    index_sha256 = receipt.get(INDEX_DIGEST_FIELD)
    if not _is_lower_hex_sha256(anchor_sha256):
        raise ReceiptError(f"{label} has invalid anchor_sha256")
    if not _is_lower_hex_sha256(index_sha256):
        raise ReceiptError(f"{label} has invalid index_sha256")
    _reject_all_zero_sha256(anchor_sha256, f"{label} anchor_sha256")
    _reject_all_zero_sha256(index_sha256, f"{label} index_sha256")
    record_count = receipt.get("record_count")
    if type(record_count) is not int or record_count < 0:
        raise ReceiptError(f"{label} record_count must be a non-negative integer")
    if require_source_files and record_count == 0:
        raise ReceiptError(
            f"{label} record_count must be positive when source files are required"
        )

    anchor_path_raw = _require_clean_path_string(
        receipt.get("anchor_path"),
        f"{label} anchor_path",
    )
    if _path_is_repository_iso_fixture(anchor_path_raw):
        raise ReceiptError(
            f"{label} anchor_path must not point to checked-in ISO fixture artifacts"
        )
    anchor_path = Path(anchor_path_raw)
    export_dir = _anchor_export_dir_for_convention(
        anchor_path,
        index_sha256=index_sha256,
        receipt_label=label,
    )
    if _path_is_symlink(anchor_path, f"{label} anchor_path"):
        raise ReceiptError(f"{label} anchor_path must not be a symlink")
    if not _path_exists(anchor_path, f"{label} anchor_path"):
        if require_source_files:
            raise ReceiptError(f"{label} references missing anchor_path")
        return (None, None)

    anchor = _load_json(
        anchor_path,
        max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES,
        display_label=anchor_label,
    )
    if type(anchor) is not dict:
        raise ReceiptError(f"{anchor_label} must contain a JSON object")
    _reject_unknown_keys(anchor, ANCHOR_KEYS, anchor_label)
    version = anchor.get("version")
    if type(version) is not int or version != ANCHOR_VERSION:
        raise ReceiptError(f"{anchor_label} has unsupported anchor version")
    if (
        require_digest_matches(anchor, ANCHOR_DIGEST_FIELD, anchor_label)
        != anchor_sha256
    ):
        raise ReceiptError(f"{label} anchor_sha256 does not match source anchor")
    if anchor.get(INDEX_DIGEST_FIELD) != index_sha256:
        raise ReceiptError(f"{label} index_sha256 does not match source anchor")
    anchor_record_count = anchor.get("record_count")
    if type(anchor_record_count) is not int or anchor_record_count < 0:
        raise ReceiptError(f"{anchor_label} record_count must be a non-negative integer")
    if anchor_record_count != record_count:
        raise ReceiptError(f"{label} record_count does not match source anchor")
    audit_index = _verify_audit_index_source(
        anchor.get("audit_index"),
        f"{anchor_label}.audit_index",
    )
    if audit_index.get(INDEX_DIGEST_FIELD) != index_sha256:
        raise ReceiptError(f"{label} index_sha256 does not match embedded audit index")
    if audit_index.get("record_count") != record_count:
        raise ReceiptError(f"{label} record_count does not match embedded audit index")
    store_dir = _record_store_dir(anchor, anchor_label)
    if store_dir is not None and _path_is_repository_iso_fixture(str(store_dir)):
        raise ReceiptError(
            f"{anchor_label}.store_dir must not point to checked-in ISO fixture artifacts"
        )
    _verify_persisted_record_sources(
        audit_index,
        store_dir,
        f"{anchor_label}.audit_index",
        require_source_files=require_source_files,
    )
    _verify_anchor_path_peers(
        anchor_path,
        export_dir,
        index_sha256=index_sha256,
        receipt_label=label,
        require_source_files=require_source_files,
    )
    index_file = export_dir / INDEX_FILE
    index_exists = _path_exists(index_file, exported_index_label)
    index_path = str(index_file) if index_exists else None
    if index_exists:
        exported_index = _verify_audit_index_source(
            _load_json(
                index_file,
                max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES,
                display_label=exported_index_label,
            ),
            exported_index_label,
        )
        if exported_index != audit_index:
            raise ReceiptError(f"{label} embedded audit index differs from exported audit index")
    elif require_source_files:
        raise ReceiptError(f"{label} references missing audit index")
    return (str(store_dir) if store_dir is not None else None, index_path)


def _normalize_optional_string(
    value: Any,
    label: str,
    *,
    allow_embedded_whitespace: bool = True,
) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str):
        raise ReceiptError(f"{label} must be null or a non-empty string")
    value = _plain_text(value, label)
    if not value.strip():
        raise ReceiptError(f"{label} must be null or a non-empty string")
    if len(value) > MAX_CLEAN_STRING_CHARS:
        raise ReceiptError(f"{label} must be no longer than {MAX_CLEAN_STRING_CHARS} characters")
    if _contains_control_character(value):
        raise ReceiptError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    if not allow_embedded_whitespace and any(ch.isspace() for ch in value):
        raise ReceiptError(f"{label} must not contain whitespace")
    return value


def _normalize_profile(value: Any, label: str) -> str | None:
    profile = _normalize_optional_string(
        value,
        label,
        allow_embedded_whitespace=False,
    )
    if profile is not None and PROFILE_ID_RE.fullmatch(profile) is None:
        raise ReceiptError(f"{label} must be a canonical lowercase profile id")
    if profile is not None:
        _reject_secret_looking_identifier(profile, label)
    return profile


def _normalize_rail_message_id(value: Any, label: str) -> str | None:
    rail_message_id = _normalize_optional_string(
        value,
        label,
        allow_embedded_whitespace=False,
    )
    if rail_message_id is None:
        return None
    if len(rail_message_id) > MAX_RAIL_MESSAGE_ID_CHARS:
        raise ReceiptError(f"{label} must be at most {MAX_RAIL_MESSAGE_ID_CHARS} characters")
    if RAIL_MESSAGE_ID_RE.fullmatch(rail_message_id) is None:
        raise ReceiptError(f"{label} must be a canonical ASCII rail message id")
    _reject_secret_looking_identifier(rail_message_id, label)
    return rail_message_id


def _normalize_sidecar_optional_string(
    sidecar: dict[str, Any],
    key: str,
    label: str,
    *,
    allow_embedded_whitespace: bool = True,
) -> str | None:
    if key not in sidecar:
        return None
    value = sidecar[key]
    if not isinstance(value, str):
        raise ReceiptError(f"{label} must be a non-empty string")
    value = _plain_text(value, label)
    if not value.strip():
        raise ReceiptError(f"{label} must be a non-empty string")
    if len(value) > MAX_CLEAN_STRING_CHARS:
        raise ReceiptError(f"{label} must be no longer than {MAX_CLEAN_STRING_CHARS} characters")
    if _contains_control_character(value):
        raise ReceiptError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    if not allow_embedded_whitespace and any(ch.isspace() for ch in value):
        raise ReceiptError(f"{label} must not contain whitespace")
    return value


def _normalize_sidecar_profile(sidecar: dict[str, Any], label: str) -> str | None:
    profile = _normalize_sidecar_optional_string(
        sidecar,
        "profile",
        label,
        allow_embedded_whitespace=False,
    )
    if profile is not None and PROFILE_ID_RE.fullmatch(profile) is None:
        raise ReceiptError(f"{label} must be a canonical lowercase profile id")
    if profile is not None:
        _reject_secret_looking_identifier(profile, label)
    return profile


def _normalize_sidecar_rail_message_id(sidecar: dict[str, Any], label: str) -> str | None:
    rail_message_id = _normalize_sidecar_optional_string(
        sidecar,
        "rail_message_id",
        label,
        allow_embedded_whitespace=False,
    )
    if rail_message_id is None:
        return None
    if len(rail_message_id) > MAX_RAIL_MESSAGE_ID_CHARS:
        raise ReceiptError(f"{label} must be at most {MAX_RAIL_MESSAGE_ID_CHARS} characters")
    if RAIL_MESSAGE_ID_RE.fullmatch(rail_message_id) is None:
        raise ReceiptError(f"{label} must be a canonical ASCII rail message id")
    _reject_secret_looking_identifier(rail_message_id, label)
    return rail_message_id


def _verify_rail_sidecar(
    receipt_label: str,
    sidecar_path: Path,
    *,
    payload_sha256: str,
    message_type: str,
    profile: str | None,
    rail_message_id: str | None,
) -> None:
    sidecar_label = f"{receipt_label} source sidecar"
    sidecar = _load_json(
        sidecar_path,
        max_bytes=MAX_RAIL_SIDECAR_JSON_BYTES,
        display_label=sidecar_label,
    )
    if type(sidecar) is not dict:
        raise ReceiptError(f"{sidecar_label} must contain a JSON object")
    _reject_unknown_keys(sidecar, RAIL_SIDECAR_KEYS, sidecar_label)
    if sidecar.get("payload_sha256") != payload_sha256:
        raise ReceiptError(f"{receipt_label} payload_sha256 does not match source sidecar")
    sidecar_message_type = sidecar.get("message_type")
    if isinstance(sidecar_message_type, str):
        _reject_non_ascii_identifier(
            sidecar_message_type,
            f"{sidecar_label} message_type",
        )
        _reject_secret_looking_identifier(
            sidecar_message_type,
            f"{sidecar_label} message_type",
        )
    if sidecar_message_type != message_type:
        raise ReceiptError(f"{receipt_label} message_type does not match source sidecar")
    sidecar_profile = _normalize_sidecar_profile(sidecar, f"{sidecar_label} profile")
    if sidecar_profile != profile:
        raise ReceiptError(f"{receipt_label} profile does not match source sidecar")
    sidecar_rail_message_id = _normalize_sidecar_rail_message_id(
        sidecar,
        f"{sidecar_label} rail_message_id",
    )
    if sidecar_rail_message_id != rail_message_id:
        raise ReceiptError(f"{receipt_label} rail_message_id does not match source sidecar")


def _verify_rail_source(
    receipt: dict[str, Any],
    path: Path,
    *,
    require_source_files: bool,
    allow_legacy_colr007: bool,
    allow_default_profile: bool,
    display_label: str | None = None,
) -> None:
    label = display_label or "receipt"
    payload_sha256 = receipt.get("payload_sha256")
    if not _is_lower_hex_sha256(payload_sha256):
        raise ReceiptError(f"{label} has invalid payload_sha256")
    _reject_all_zero_sha256(payload_sha256, f"{label} payload_sha256")
    message_type = _require_clean_string(receipt.get("message_type"), f"{label} message_type")
    _reject_non_ascii_identifier(message_type, f"{label} message_type")
    _reject_secret_looking_identifier(message_type, f"{label} message_type")
    if MESSAGE_TYPE_RE.fullmatch(message_type) is None:
        raise ReceiptError(f"{label} message_type must be lowercase ISO family id")
    if message_type not in SUPPORTED_RAIL_MESSAGE_TYPES:
        raise ReceiptError(f"{label} has unsupported rail message_type")
    if message_type in LEGACY_RAIL_MESSAGE_TYPES and not allow_legacy_colr007:
        raise ReceiptError(
            f"{label} uses legacy rail message_type; "
            "production evidence must use colr.012"
        )
    if "profile" not in receipt:
        raise ReceiptError(f"{label} profile must be recorded")
    profile = _normalize_profile(receipt["profile"], f"{label} profile")
    if profile is None and not allow_default_profile:
        raise ReceiptError(f"{label} omitted rail profile")
    if "rail_message_id" not in receipt:
        raise ReceiptError(f"{label} rail_message_id must be recorded")
    rail_message_id = _normalize_rail_message_id(
        receipt["rail_message_id"],
        f"{label} rail_message_id",
    )

    xml_path_raw = _require_clean_path_string(receipt.get("xml_path"), f"{label} xml_path")
    xml_path = Path(xml_path_raw)
    sidecar_path_raw = _require_clean_path_string(
        receipt.get("sidecar_path"),
        f"{label} sidecar_path",
    )
    sidecar_path = Path(sidecar_path_raw)
    if xml_path.suffix != ".xml":
        raise ReceiptError(f"{label} xml_path must point to a .xml file")
    if _path_is_repository_iso_fixture(xml_path_raw):
        raise ReceiptError(
            f"{label} xml_path must not point to checked-in ISO XML fixtures"
        )
    expected_sidecar = xml_path.with_suffix(xml_path.suffix + ".json")
    if _path_resolve(sidecar_path, f"{label} sidecar_path") != _path_resolve(
        expected_sidecar,
        f"{label} expected sidecar_path",
    ):
        raise ReceiptError(f"{label} sidecar_path must match xml_path sidecar")
    if _path_exists(sidecar_path, f"{label} sidecar_path"):
        _verify_rail_sidecar(
            label,
            sidecar_path,
            payload_sha256=payload_sha256,
            message_type=message_type,
            profile=profile,
            rail_message_id=rail_message_id,
        )
    elif require_source_files:
        raise ReceiptError(f"{label} references missing sidecar_path")
    if not _path_exists(xml_path, f"{label} xml_path"):
        if require_source_files:
            raise ReceiptError(f"{label} references missing xml_path")
        return

    actual = sha256_hex(
        _read_regular_file(
            xml_path,
            max_bytes=MAX_RAIL_XML_BYTES,
            limit_label="payload",
            display_label=f"{label} source XML",
        )
    )
    if actual != payload_sha256:
        raise ReceiptError(f"{label} payload_sha256 does not match source XML")


def verify_receipt_file(
    path: Path,
    *,
    allow_failed: bool,
    allow_insecure_http: bool,
    allow_legacy_colr007: bool,
    allow_default_profile: bool,
    require_source_files: bool,
    display_label: str | None = None,
) -> dict[str, Any]:
    """Verify one operator receipt and return its parsed JSON object."""

    label = display_label or "receipt"
    receipt = _load_json(
        path,
        max_bytes=MAX_RECEIPT_JSON_BYTES,
        display_label=label,
    )
    if type(receipt) is not dict:
        raise ReceiptError(f"{label} must contain a JSON object")
    _check_no_secret_material(receipt, label)
    version = receipt.get("version")
    if type(version) is not int or version != RECEIPT_VERSION:
        raise ReceiptError(f"{label} has unsupported receipt version")
    kind = receipt.get("receipt_kind")
    if isinstance(kind, str):
        _reject_non_ascii_identifier(kind, f"{label} receipt_kind")
        _reject_secret_looking_identifier(kind, f"{label} receipt_kind")
    if kind not in SUPPORTED_KINDS:
        raise ReceiptError(f"{label} has unsupported receipt_kind")
    _reject_unknown_keys(receipt, RECEIPT_KEYS_BY_KIND[kind], label)
    require_digest_matches(receipt, RECEIPT_DIGEST_FIELD, label)
    _check_status(receipt, label, allow_failed=allow_failed)
    _check_response_metadata(receipt, label, kind=kind)

    if kind == "iso-audit-notary":
        _check_timestamp(receipt, "published_at", label)
        endpoint = _require_clean_string(receipt.get("endpoint"), f"{label} endpoint")
        _require_https(endpoint, allow_insecure_http=allow_insecure_http, label=label)
        _check_endpoint_digest(receipt, label, endpoint)
        verified_store_dir, verified_index_path = _verify_anchor_source(
            receipt,
            path,
            require_source_files=require_source_files,
            display_label=label,
        )
        receipt["_verified_store_dir"] = verified_store_dir
        receipt["_verified_index_path"] = verified_index_path
    elif kind == "iso-rail-gateway":
        _check_timestamp(receipt, "submitted_at", label)
        endpoint_url = _require_clean_string(
            receipt.get("endpoint_url"),
            f"{label} endpoint_url",
        )
        _require_https(endpoint_url, allow_insecure_http=allow_insecure_http, label=label)
        _check_endpoint_digest(receipt, label, endpoint_url)
        _verify_rail_source(
            receipt,
            path,
            require_source_files=require_source_files,
            allow_legacy_colr007=allow_legacy_colr007,
            allow_default_profile=allow_default_profile,
            display_label=label,
        )
    else:  # pragma: no cover - guarded above, kept explicit for future kinds.
        raise ReceiptError(f"{label} has unsupported receipt_kind")

    return receipt


def discover_receipts(receipt_dir: Path, *, display_label: str | None = None) -> list[Path]:
    """Return receipt files in deterministic order."""

    label = display_label or "receipt_dir"
    _reject_repository_iso_fixture_path(receipt_dir, label)
    _reject_symlinked_existing_ancestors(receipt_dir.parent, display_label=label)
    try:
        metadata = receipt_dir.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{label} does not exist") from error
    except OSError as error:
        detail = _safe_os_error_detail(error)
        raise ReceiptError(f"cannot inspect {label}: {detail}") from error
    except (RuntimeError, TypeError, ValueError):
        raise ReceiptError(f"cannot inspect {label}: I/O error") from None
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise ReceiptError(f"{label} is not a directory")
    receipts = sorted(receipt_dir.glob("*.receipt.json"))
    if not receipts:
        raise ReceiptError(f"{label} has no *.receipt.json files")
    return receipts


def _reject_duplicate_paths(paths: list[Path]) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(_path_resolve(path, f"receipt[{offset}]"))
        if key in seen:
            raise ReceiptError(f"receipt[{offset}] duplicates receipt[{seen[key]}]")
        seen[key] = offset


def _receipt_metadata(path: Path, receipt: dict[str, Any]) -> dict[str, Any]:
    metadata: dict[str, Any] = {
        "path": str(path),
        "receipt_kind": receipt["receipt_kind"],
        "receipt_sha256": receipt[RECEIPT_DIGEST_FIELD],
        "ok": receipt.get("ok"),
        "status_code": receipt.get("status_code"),
        "response_body_sha256": receipt.get("response_body_sha256"),
        "endpoint_requires_insecure_http": _url_requires_insecure_http_override(
            urllib.parse.urlparse(_receipt_endpoint_url(receipt))
        ),
    }
    if receipt["receipt_kind"] == "iso-audit-notary":
        metadata.update(
            {
                "anchor_path": receipt.get("anchor_path"),
                "store_dir": receipt.get("_verified_store_dir"),
                "index_path": receipt.get("_verified_index_path"),
                "anchor_sha256": receipt.get(ANCHOR_DIGEST_FIELD),
                "index_sha256": receipt.get(INDEX_DIGEST_FIELD),
                "record_count": receipt.get("record_count"),
            }
        )
    elif receipt["receipt_kind"] == "iso-rail-gateway":
        metadata.update(
            {
                "message_type": receipt.get("message_type"),
                "payload_sha256": receipt.get("payload_sha256"),
                "profile": receipt.get("profile"),
                "rail_message_id": receipt.get("rail_message_id"),
                "source_path": receipt.get("xml_path"),
            }
        )
    return metadata


def _receipt_summary_entry_order_key(entry: dict[str, Any]) -> tuple[str, str, str]:
    return (entry["receipt_kind"], entry["path"], entry["receipt_sha256"])


def _receipt_endpoint_url(receipt: dict[str, Any]) -> str:
    if receipt["receipt_kind"] == "iso-audit-notary":
        return receipt["endpoint"]
    if receipt["receipt_kind"] == "iso-rail-gateway":
        return receipt["endpoint_url"]
    raise ReceiptError("unsupported receipt_kind")


def _reject_unused_local_overrides(args: argparse.Namespace, receipts: list[dict[str, Any]]) -> None:
    if args.allow_failed and not any(receipt.get("ok") is False for receipt in receipts):
        raise ReceiptError("--allow-failed requires at least one failed receipt")
    if args.allow_insecure_http and not any(
        _url_requires_insecure_http_override(
            urllib.parse.urlparse(_receipt_endpoint_url(receipt))
        )
        for receipt in receipts
    ):
        raise ReceiptError(
            "--allow-insecure-http requires at least one http:// or local/private receipt endpoint"
        )
    if args.allow_legacy_colr007 and not any(
        receipt.get("receipt_kind") == "iso-rail-gateway"
        and receipt.get("message_type") in LEGACY_RAIL_MESSAGE_TYPES
        for receipt in receipts
    ):
        raise ReceiptError(
            "--allow-legacy-colr007 requires at least one rail receipt with legacy colr.007 message_type"
        )
    if args.allow_default_profile and not any(
        receipt.get("receipt_kind") == "iso-rail-gateway"
        and receipt.get("profile") is None
        for receipt in receipts
    ):
        raise ReceiptError(
            "--allow-default-profile requires at least one rail receipt without an explicit profile"
        )


def _require_policy_booleans(args: argparse.Namespace) -> None:
    for attr in (
        "allow_failed",
        "allow_insecure_http",
        "allow_legacy_colr007",
        "allow_default_profile",
        "require_source_files",
    ):
        flag = f"--{attr.replace('_', '-')}"
        setattr(args, attr, _require_bool(getattr(args, attr, None), flag))


def _required_cli_path_sequence(value: Any, label: str) -> list[Path]:
    if value is None:
        return []
    if isinstance(value, (str, bytes)) or type(value) not in (list, tuple):
        raise ReceiptError(f"{label} must be a repeatable path list")
    if len(value) > MAX_RECEIPT_INPUT_PATHS:
        raise ReceiptError(f"{label} accepts at most {MAX_RECEIPT_INPUT_PATHS} paths")
    paths: list[Path] = []
    for offset, entry in enumerate(value):
        if isinstance(entry, bytes):
            raise ReceiptError(f"{label}[{offset}] must be a path")
        if isinstance(entry, str):
            entry = _plain_text(entry, f"{label}[{offset}]")
            paths.append(Path(entry))
        elif type(entry) is type(Path()):
            paths.append(Path(entry))
        else:
            raise ReceiptError(f"{label}[{offset}] must be a path")
    return paths


def run(args: argparse.Namespace) -> int:
    args = _require_plain_namespace(args)
    _require_policy_booleans(args)
    receipt_paths = _required_cli_path_sequence(getattr(args, "receipt", None), "--receipt")
    receipt_dir_paths = _required_cli_path_sequence(
        getattr(args, "receipt_dir", None),
        "--receipt-dir",
    )
    for offset, path in enumerate(receipt_paths):
        _reject_raw_cli_path_smuggling(str(path), f"receipt[{offset}]")
        _reject_repository_iso_fixture_path(path, f"receipt[{offset}]")
    for offset, receipt_dir in enumerate(receipt_dir_paths):
        _reject_raw_cli_path_smuggling(str(receipt_dir), f"receipt_dir[{offset}]")
        _reject_repository_iso_fixture_path(receipt_dir, f"receipt_dir[{offset}]")
    paths = list(receipt_paths)
    for offset, receipt_dir in enumerate(receipt_dir_paths):
        paths.extend(discover_receipts(receipt_dir, display_label=f"receipt_dir[{offset}]"))
    if not paths:
        raise ReceiptError("provide at least one --receipt or --receipt-dir")
    _reject_duplicate_paths(paths)

    verified: list[dict[str, Any]] = []
    receipt_entries: list[dict[str, Any]] = []
    seen_receipt_digests: dict[str, int] = {}
    for offset, path in enumerate(paths):
        receipt = verify_receipt_file(
            path,
            allow_failed=args.allow_failed,
            allow_insecure_http=args.allow_insecure_http,
            allow_legacy_colr007=args.allow_legacy_colr007,
            allow_default_profile=args.allow_default_profile,
            require_source_files=args.require_source_files,
            display_label=f"receipt[{offset}]",
        )
        receipt_digest = receipt[RECEIPT_DIGEST_FIELD]
        if receipt_digest in seen_receipt_digests:
            raise ReceiptError(
                f"receipt[{offset}] {RECEIPT_DIGEST_FIELD} duplicates "
                f"receipt[{seen_receipt_digests[receipt_digest]}]"
            )
        seen_receipt_digests[receipt_digest] = offset
        verified.append(receipt)
        receipt_entries.append(_receipt_metadata(path, receipt))

    _reject_unused_local_overrides(args, verified)
    receipt_entries.sort(key=_receipt_summary_entry_order_key)

    summary = {
        "version": RECEIPT_SUMMARY_VERSION,
        "verified_receipts": len(verified),
        "receipt_kind": sorted({receipt["receipt_kind"] for receipt in verified}),
        "allow_failed": args.allow_failed,
        "allow_insecure_http": args.allow_insecure_http,
        "allow_legacy_colr007": args.allow_legacy_colr007,
        "allow_default_profile": args.allow_default_profile,
        "require_source_files": args.require_source_files,
        "receipts": receipt_entries,
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_summary_json_bytes(summary))
    print(json.dumps(summary, allow_nan=False, indent=2, sort_keys=True))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog=Path(__file__).name,
        description="Verify ISO 20022 operator rail/notary adapter receipts.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--receipt",
        action="append",
        default=[],
        type=Path,
        help="Receipt JSON file to verify; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        action="append",
        default=[],
        type=Path,
        help="Directory containing *.receipt.json files to verify.",
    )
    parser.add_argument(
        "--allow-failed",
        action="store_true",
        help="Allow receipts whose remote submission/publication failed.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// endpoints in receipts for local tests.",
    )
    parser.add_argument(
        "--allow-legacy-colr007",
        action="store_true",
        help="Allow legacy local colr.007 rail receipts; production evidence should use colr.012.",
    )
    parser.add_argument(
        "--allow-default-profile",
        action="store_true",
        help="Allow rail receipts that omitted an explicit profile for local tests.",
    )
    parser.add_argument(
        "--require-source-files",
        action="store_true",
        help="Require referenced source XML/anchor files to exist and match receipt digests.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    try:
        normalised_argv = _normalise_cli_argv(argv)
        parser = build_parser()
        _preflight_raw_cli_secrets(normalised_argv, {"--receipt", "--receipt-dir"})
        _preflight_boolean_cli_flags(
            normalised_argv,
            {
                "--allow-default-profile",
                "--allow-failed",
                "--allow-insecure-http",
                "--allow-legacy-colr007",
                "--require-source-files",
            },
        )
        _preflight_cli_paths(normalised_argv, {"--receipt", "--receipt-dir"})
        args = parser.parse_args(normalised_argv)
        return run(args)
    except ReceiptError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
