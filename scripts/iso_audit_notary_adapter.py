#!/usr/bin/env python3
"""Publish ISO 20022 audit export anchors to external archival/notary services.

Purpose:
  This operator-side adapter consumes the digest-bound preimages written by
  Torii's ``iso_bridge.audit_export_dir``. It verifies the local
  ``messages.index.json`` and ``*.notary.json`` preimages before publishing them
  to configured HTTPS endpoints, then writes bounded local receipts.

Prerequisites:
  Python 3.11+ and a populated ISO audit export directory from Torii. No third
  party Python packages are required.

Safety:
  The script never mutates Torii state and never deletes files. Plain HTTP
  endpoints are rejected unless ``--allow-insecure-http`` is supplied for local
  tests. Bearer tokens are read from a runtime-only file and are never persisted
  into receipts.
"""

from __future__ import annotations

import argparse
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
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


ANCHOR_DIGEST_FIELD = "anchor_sha256"
ANCHOR_DIR = "anchors"
ANCHOR_VERSION = 1
PERSISTED_RECORD_DIGEST_FIELD = "record_sha256"
PERSISTED_RECORD_VERSION = 1
RECORDS_DIR = "messages"
MAX_BEARER_TOKEN_BYTES = 8192
MAX_HTTP_URL_CHARS = 2048
MAX_AUDIT_EXPORT_JSON_BYTES = 64 * 1024 * 1024
LOCAL_REBINDING_HOST_SUFFIXES = {"localtest.me", "lvh.me", "nip.io", "sslip.io", "vcap.me"}
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
DEFAULT_RESPONSE_LIMIT_BYTES = 64 * 1024
INDEX_DIGEST_FIELD = "index_sha256"
INDEX_FILE = "messages.index.json"
INDEX_VERSION = 1
LATEST_ANCHOR_FILE = "latest.notary.json"
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
AUDIT_INDEX_KEYS = {
    "version",
    "record_count",
    "records",
    INDEX_DIGEST_FIELD,
}
PACS002_CODES = {"ACTC", "ACSP", "ACSC", "ACWC", "PDNG", "RJCT"}
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
SECRET_PREVIEW_MARKERS = (
    "authorization",
    "bearer ",
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
    "secret",
    "token",
    "x_iroha_signature",
)
SECRET_VALUE_PATTERNS = [
    re.compile(r"\bauthorization\s*:", re.IGNORECASE),
    re.compile(r"\bbearer\s+[A-Za-z0-9._~+/=-]+", re.IGNORECASE),
    re.compile(
        r"\b(?:token|secret|private[_-]?key|password|passphrase|api[_-]?key|access[_-]?key|session[_-]?key|client[_-]?secret|cookie|set-cookie)\s*[:=]\s*\S+",
        re.IGNORECASE,
    ),
    re.compile(r"\bx-iroha-signature\s*:", re.IGNORECASE),
]
REDACTED_RESPONSE_PREVIEW = "[redacted: sensitive response body]"
REDACTED_ERROR = "[redacted: sensitive error]"


class _NoRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *_args: object, **_kwargs: object) -> None:
        return None


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


NO_REDIRECT_OPENER = urllib.request.build_opener(_NoRedirectHandler)


class AdapterError(RuntimeError):
    """Raised when an audit preimage or publication response is invalid."""


def _require_positive_cli_int(value: int, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise AdapterError(f"{label} must be a positive integer")
    return value


def _require_positive_finite_cli_number(value: float, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise AdapterError(f"{label} must be a positive finite number")
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise AdapterError(f"{label} must be a positive finite number")
    return parsed


@dataclass(frozen=True)
class VerifiedAnchor:
    """Verified anchor bytes and selected metadata ready for publication."""

    path: Path
    payload: dict[str, Any]
    raw: bytes
    index_sha256: str
    anchor_sha256: str
    record_count: int


@dataclass(frozen=True)
class PublishResult:
    """Publication outcome for one endpoint."""

    endpoint: str
    status_code: int | None
    ok: bool
    response_body_sha256: str | None
    response_body_preview: str | None
    error: str | None = None


def _read_regular_file(
    path: Path,
    *,
    max_bytes: int | None = None,
    limit_label: str = "input",
    path_label: str | None = None,
) -> bytes:
    if max_bytes is not None and (
        isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or max_bytes <= 0
    ):
        raise AdapterError("max file bytes must be a positive integer")
    display_path = path_label if path_label is not None else str(path)
    try:
        _reject_symlinked_existing_ancestors(path.parent)
    except AdapterError as error:
        if path_label is not None:
            raise AdapterError(f"{display_path} ancestor must not be a symlink") from error
        raise
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{display_path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{display_path} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise AdapterError(f"{display_path} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise AdapterError(f"{display_path} exceeds {max_bytes} byte {limit_label} limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise AdapterError(f"{display_path} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise AdapterError(f"{display_path} exceeds {max_bytes} byte {limit_label} limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise AdapterError(f"{display_path} exceeds {max_bytes} byte {limit_label} limit")
        return raw
    except FileNotFoundError as error:
        raise AdapterError(f"{display_path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise AdapterError(f"{display_path} must not be a symlink") from error
        raise AdapterError(f"cannot open {display_path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _ensure_input_directory(path: Path, label: str) -> None:
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{label} {path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{label} {path} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise AdapterError(f"{label} {path} must be a directory")


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
            raise AdapterError(f"{current} must not be a symlink")


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise AdapterError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise AdapterError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise AdapterError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise AdapterError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise AdapterError(f"{label} must use forward slashes")
    if ";" in raw:
        raise AdapterError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(raw) or _is_secret_looking_key(raw):
        raise AdapterError(f"{label} must not contain secret-looking material")
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise AdapterError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise AdapterError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise AdapterError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise AdapterError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise AdapterError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise AdapterError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise AdapterError(f"{label} must use forward slashes")
    if ";" in raw:
        raise AdapterError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(raw) or _is_secret_looking_key(raw):
        raise AdapterError(f"{label} must not contain secret-looking material")
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise AdapterError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise AdapterError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise AdapterError(f"{label} must not contain dot or parent segments")


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
        if _contains_secret_material(arg) or _is_secret_looking_key(arg):
            raise AdapterError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise AdapterError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise AdapterError(f"{arg} does not take a value")
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
                    raise AdapterError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 1
                matched = True
                break
        if not matched:
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
                    raise AdapterError(f"{flag} requires a {value_name} value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a {value_name} value")
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a {value_name} value")
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise AdapterError(f"{flag} must be a numeric value")
    try:
        int(raw, 10) if integer else float(raw)
    except ValueError as error:
        raise AdapterError(f"{flag} must be a numeric value") from error


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
                    raise AdapterError(f"{flag} requires a numeric value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _ensure_output_directory(path: Path, label: str) -> None:
    _reject_output_path_smuggling(path, label)
    _reject_symlinked_existing_ancestors(path)
    if path.exists() or path.is_symlink():
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise AdapterError(f"{label} {path} must not be a symlink")
        if not stat.S_ISDIR(mode):
            raise AdapterError(f"{label} {path} must be a directory")
        return
    path.mkdir(parents=True, exist_ok=True)
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise AdapterError(f"{label} {path} must not be a symlink")
    if not stat.S_ISDIR(mode):
        raise AdapterError(f"{label} {path} must be a directory")


def _ensure_output_file_target(path: Path) -> None:
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise AdapterError(f"{path} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise AdapterError(f"{path} must be a regular file")
        if metadata.st_nlink > 1:
            raise AdapterError(f"{path} must not be hard-linked")


def _write_text_output(path: Path, text: str) -> None:
    _reject_output_path_smuggling(path, "output path")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except FileExistsError as error:
        raise AdapterError(f"{path.parent} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise AdapterError(f"{path.parent} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise AdapterError(f"{path.parent} must be a directory")
    _ensure_output_file_target(path)
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise AdapterError(f"{path.parent} must not be a symlink") from error
        raise AdapterError(f"{path.parent} must be a directory") from error

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
                raise AdapterError(f"{path} temp file must not be a symlink") from error
            raise AdapterError(
                f"cannot open temporary output for {path}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise AdapterError(f"{path} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise AdapterError(f"{path} temp file must not be hard-linked")
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


def _absolute_path_without_resolving_leaf(path: Path) -> Path:
    return path if path.is_absolute() else Path.cwd() / path


def _load_json(path: Path) -> Any:
    raw = _read_regular_file(path, max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES)
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error
    _reject_json_surrogates(value)
    return value


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise AdapterError("JSON object contains duplicate key")
        seen.add(key)
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise AdapterError(f"JSON contains non-finite numeric constant {value}")


def _reject_json_surrogates(value: Any) -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise AdapterError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        for item in value:
            _reject_json_surrogates(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            _reject_json_surrogates(key)
            _reject_json_surrogates(item)


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        if any(_is_secret_looking_key(key) for key in unknown):
            raise AdapterError(f"{label} contains unknown keys")
        raise AdapterError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _is_secret_looking_key(value: Any) -> bool:
    markers = (
        "authorization",
        "bearer",
        "token",
        "secret",
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
    return any(
        marker in candidate.lower()
        for candidate in _secret_scan_values(str(value))
        for marker in markers
    )


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise AdapterError(f"{label} must not contain secret-looking material")


def _load_json_bytes(path: Path) -> tuple[Any, bytes]:
    raw = _read_regular_file(path, max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES)
    try:
        value = json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error
    _reject_json_surrogates(value)
    return value, raw


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _is_lower_hex_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def digest_without_field(obj: dict[str, Any], digest_field: str) -> str:
    """Compute the same JSON-object digest shape used by Torii export code."""

    if digest_field not in obj:
        raise AdapterError(f"missing {digest_field}")
    body = dict(obj)
    body.pop(digest_field)
    return sha256_hex(_canonical_json_bytes(body))


def require_digest_matches(obj: dict[str, Any], digest_field: str, label: str) -> str:
    """Validate and return an embedded digest field."""

    expected = obj.get(digest_field)
    if not _is_lower_hex_sha256(expected):
        raise AdapterError(f"{label} has missing or non-canonical {digest_field}")
    actual = digest_without_field(obj, digest_field)
    if actual != expected:
        raise AdapterError(f"{label} {digest_field} mismatch: expected {expected}, got {actual}")
    return expected


def _require_clean_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise AdapterError(f"{label} must be a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise AdapterError(f"{label} must not contain control characters")
    if value != value.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    return value


def _require_clean_path_string(value: Any, label: str) -> str:
    path = _require_clean_string(value, label)
    if any(ch.isspace() for ch in path):
        raise AdapterError(f"{label} must not contain whitespace")
    if path.startswith("-"):
        raise AdapterError(f"{label} must not start with a dash")
    if "\\" in path:
        raise AdapterError(f"{label} must use forward slashes")
    if ";" in path:
        raise AdapterError(f"{label} must not contain semicolon path parameters")
    if _contains_secret_material(path) or _is_secret_looking_key(path):
        raise AdapterError(f"{label} must not contain secret-looking material")
    parts = path.split("/")
    checked_parts = parts[1:] if path.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise AdapterError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise AdapterError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise AdapterError(f"{label} must not contain dot or parent segments")
    return path


def _require_optional_clean_string(value: Any, label: str) -> str | None:
    if value is None:
        return None
    return _require_clean_string(value, label)


def _require_nonnegative_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
        raise AdapterError(f"{label} must be a non-negative integer")
    return value


def _require_optional_nonnegative_int(value: Any, label: str) -> int | None:
    if value is None:
        return None
    return _require_nonnegative_int(value, label)


def _require_bool(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise AdapterError(f"{label} must be a boolean")
    return value


def _expected_message_filename(message_id: str) -> str:
    return f"{sha256_hex(message_id.encode('utf-8'))}.json"


def _require_pacs002_code(value: Any, label: str) -> str:
    code = _require_clean_string(value, label)
    if code not in PACS002_CODES:
        raise AdapterError(f"{label} must be a supported pacs.002 status code")
    return code


def _derived_pacs002_code(record: dict[str, Any], label: str) -> str:
    state = _require_clean_string(record.get("state"), f"{label}.state")
    if state not in {"Pending", "Accepted", "Rejected"}:
        raise AdapterError(f"{label}.state must be Pending, Accepted, or Rejected")
    if state == "Rejected":
        return "RJCT"
    if state == "Accepted":
        return "ACSC" if record.get("settled_at_ms") is not None else "ACSP"
    if record.get("hold_reason_code") is not None:
        return "PDNG"
    change_reason_codes = record.get("change_reason_codes")
    if isinstance(change_reason_codes, list) and change_reason_codes:
        return "ACWC"
    if record.get("ledger_tx_queued"):
        return "ACSP"
    return "ACTC"


def _verify_optional_clean_string_fields(
    value: dict[str, Any], keys: set[str], label: str
) -> None:
    for key in keys:
        _require_optional_clean_string(value.get(key), f"{label}.{key}")


def _verify_persisted_context(value: Any, label: str) -> None:
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_CONTEXT_KEYS, label)
    _verify_optional_clean_string_fields(value, PERSISTED_CONTEXT_KEYS, label)


def _verify_persisted_metadata(
    value: Any, label: str, index_record: dict[str, Any]
) -> None:
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_METADATA_KEYS, label)
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
        raise AdapterError(f"{label}.payload_hash must be a canonical SHA-256")
    for key in (
        "profile_id",
        "message_type",
        "business_message_id",
        "uetr",
        "payload_hash",
        "reference_snapshot_id",
    ):
        if value.get(key) != index_record.get(key):
            raise AdapterError(f"{label}.{key} does not match audit index record")


def _verify_persisted_history_entry(value: Any, label: str) -> tuple[str, str, int]:
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_HISTORY_KEYS, label)
    status = _require_clean_string(value.get("status"), f"{label}.status")
    if status not in {"Pending", "Accepted", "Rejected"}:
        raise AdapterError(f"{label}.status must be Pending, Accepted, or Rejected")
    code = _require_pacs002_code(value.get("pacs002_code"), f"{label}.pacs002_code")
    updated_at_ms = _require_nonnegative_int(
        value.get("updated_at_ms"),
        f"{label}.updated_at_ms",
    )
    _require_optional_clean_string(value.get("detail"), f"{label}.detail")
    _require_optional_clean_string(value.get("reason_code"), f"{label}.reason_code")
    return status, code, updated_at_ms


def _verify_persisted_record_source(
    index_record: dict[str, Any],
    path: Path,
    label: str,
) -> None:
    value = _load_json(path)
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must contain a JSON object")
    _reject_unknown_keys(value, PERSISTED_RECORD_KEYS, label)
    version = value.get("version")
    if (
        isinstance(version, bool)
        or not isinstance(version, int)
        or version != PERSISTED_RECORD_VERSION
    ):
        raise AdapterError(f"{label} has unsupported persisted record version")
    source_digest = require_digest_matches(value, PERSISTED_RECORD_DIGEST_FIELD, label)
    if source_digest != index_record.get(PERSISTED_RECORD_DIGEST_FIELD):
        raise AdapterError(f"{label} record_sha256 does not match audit index record")
    if value.get("message_id") != index_record.get("message_id"):
        raise AdapterError(f"{label}.message_id does not match audit index record")
    if value.get("state") != index_record.get("state"):
        raise AdapterError(f"{label}.state does not match audit index record")
    updated_at_ms = _require_nonnegative_int(
        value.get("updated_at_ms"),
        f"{label}.updated_at_ms",
    )
    if updated_at_ms != index_record.get("updated_at_ms"):
        raise AdapterError(f"{label}.updated_at_ms does not match audit index record")
    _require_optional_nonnegative_int(value.get("settled_at_ms"), f"{label}.settled_at_ms")
    if value.get("settled_at_ms") != index_record.get("settled_at_ms"):
        raise AdapterError(f"{label}.settled_at_ms does not match audit index record")
    for key in (
        "transaction_hash",
        "detail",
        "hold_reason_code",
        "rejection_reason_code",
    ):
        _require_optional_clean_string(value.get(key), f"{label}.{key}")
    if value.get("transaction_hash") != index_record.get("transaction_hash"):
        raise AdapterError(f"{label}.transaction_hash does not match audit index record")
    _require_bool(value.get("ledger_tx_queued"), f"{label}.ledger_tx_queued")
    change_reason_codes = value.get("change_reason_codes")
    if not isinstance(change_reason_codes, list):
        raise AdapterError(f"{label}.change_reason_codes must be an array")
    for offset, code in enumerate(change_reason_codes):
        _require_clean_string(code, f"{label}.change_reason_codes[{offset}]")
    _verify_persisted_context(value.get("context"), f"{label}.context")
    _verify_persisted_metadata(
        value.get("metadata"),
        f"{label}.metadata",
        index_record,
    )
    history = value.get("status_history")
    if not isinstance(history, list) or not history:
        raise AdapterError(f"{label}.status_history must be a non-empty array")
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
            raise AdapterError(
                f"{label}.status_history[{offset}].updated_at_ms must not move backwards"
            )
        previous_updated_at_ms = last_updated_at_ms
    derived_code = _derived_pacs002_code(value, label)
    if derived_code != index_record.get("pacs002_code"):
        raise AdapterError(f"{label}.pacs002_code does not match persisted state")
    if last_status != value.get("state") or last_code != derived_code:
        raise AdapterError(f"{label}.status_history does not end with current status")
    if last_updated_at_ms != updated_at_ms:
        raise AdapterError(f"{label}.status_history does not end at current updated_at_ms")


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
    allow_missing_record_sources: bool,
) -> None:
    records = audit_index.get("records", [])
    if store_dir is None:
        if not allow_missing_record_sources and records:
            raise AdapterError(f"{label}.store_dir is required to verify audit records")
        return
    if not store_dir.exists():
        if not allow_missing_record_sources:
            raise AdapterError(f"{label}.store_dir {store_dir} does not exist")
        return
    _ensure_input_directory(store_dir, f"{label}.store_dir")
    messages_dir = store_dir / RECORDS_DIR
    if not messages_dir.exists():
        if not allow_missing_record_sources:
            raise AdapterError(
                f"{label}.store_dir/{RECORDS_DIR} {messages_dir} does not exist"
            )
        return
    _ensure_input_directory(messages_dir, f"{label}.store_dir/{RECORDS_DIR}")
    for offset, record in enumerate(records):
        if not isinstance(record, dict):
            raise AdapterError(f"{label}.records[{offset}] must be an object")
        record_path = messages_dir / record["filename"]
        if not record_path.exists() and allow_missing_record_sources:
            continue
        _verify_persisted_record_source(
            record,
            record_path,
            f"{record_path}",
        )


def _verify_audit_index_record(record: Any, label: str) -> None:
    if not isinstance(record, dict):
        raise AdapterError(f"{label} must be an object")
    _reject_unknown_keys(record, AUDIT_INDEX_RECORD_KEYS, label)
    message_id = _require_clean_string(record.get("message_id"), f"{label}.message_id")
    filename = _require_clean_string(record.get("filename"), f"{label}.filename")
    expected_filename = _expected_message_filename(message_id)
    if filename != expected_filename:
        raise AdapterError(
            f"{label}.filename must be digest-addressed as {expected_filename}"
        )
    if not _is_lower_hex_sha256(record.get("record_sha256")):
        raise AdapterError(f"{label}.record_sha256 must be a canonical SHA-256")
    _require_clean_string(record.get("state"), f"{label}.state")
    _require_pacs002_code(record.get("pacs002_code"), f"{label}.pacs002_code")
    _require_nonnegative_int(record.get("updated_at_ms"), f"{label}.updated_at_ms")
    _require_optional_nonnegative_int(record.get("settled_at_ms"), f"{label}.settled_at_ms")
    _require_optional_clean_string(record.get("transaction_hash"), f"{label}.transaction_hash")
    _require_optional_clean_string(record.get("profile_id"), f"{label}.profile_id")
    _require_optional_clean_string(record.get("message_type"), f"{label}.message_type")
    _require_optional_clean_string(
        record.get("business_message_id"),
        f"{label}.business_message_id",
    )
    _require_optional_clean_string(record.get("uetr"), f"{label}.uetr")
    payload_hash = _require_optional_clean_string(
        record.get("payload_hash"),
        f"{label}.payload_hash",
    )
    if payload_hash is not None and not _is_lower_hex_sha256(payload_hash):
        raise AdapterError(f"{label}.payload_hash must be a canonical SHA-256")
    _require_optional_clean_string(
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
                raise AdapterError(
                    f"{label} records[{offset}].{field} duplicates "
                    f"{label} records[{field_seen[value]}].{field}"
                )
            field_seen[value] = offset


def verify_audit_index(index: Any) -> dict[str, Any]:
    """Verify the exported audit index digest and basic record-count shape."""

    if not isinstance(index, dict):
        raise AdapterError("audit index must be a JSON object")
    _reject_unknown_keys(index, AUDIT_INDEX_KEYS, "audit index")
    version = index.get("version")
    if isinstance(version, bool) or not isinstance(version, int) or version != INDEX_VERSION:
        raise AdapterError(f"audit index version must be {INDEX_VERSION}")
    require_digest_matches(index, INDEX_DIGEST_FIELD, "audit index")
    record_count = index.get("record_count")
    records = index.get("records")
    if isinstance(record_count, bool) or not isinstance(record_count, int) or record_count < 0:
        raise AdapterError("audit index record_count must be a non-negative integer")
    if not isinstance(records, list):
        raise AdapterError("audit index records must be an array")
    if len(records) != record_count:
        raise AdapterError(
            f"audit index record_count {record_count} does not match records length {len(records)}"
        )
    for offset, record in enumerate(records):
        _verify_audit_index_record(record, f"audit index records[{offset}]")
    _reject_duplicate_audit_index_records(records, "audit index")
    return index


def verify_anchor_file(
    export_dir: Path,
    anchor_path: Path,
    *,
    allow_missing_record_sources: bool = False,
) -> VerifiedAnchor:
    """Verify one notary anchor against the export directory index file."""

    _reject_raw_output_path_smuggling(str(anchor_path), "anchor path")
    anchor_value, raw = _load_json_bytes(anchor_path)
    if not isinstance(anchor_value, dict):
        raise AdapterError(f"{anchor_path} must contain a JSON object")
    _reject_unknown_keys(anchor_value, ANCHOR_KEYS, str(anchor_path))
    version = anchor_value.get("version")
    if isinstance(version, bool) or not isinstance(version, int) or version != ANCHOR_VERSION:
        raise AdapterError(f"{anchor_path} has unsupported anchor version")
    anchor_sha256 = require_digest_matches(anchor_value, ANCHOR_DIGEST_FIELD, str(anchor_path))

    audit_index = verify_audit_index(anchor_value.get("audit_index"))
    index_sha256 = anchor_value.get(INDEX_DIGEST_FIELD)
    embedded_index_sha256 = audit_index.get(INDEX_DIGEST_FIELD)
    if index_sha256 != embedded_index_sha256:
        raise AdapterError(
            f"{anchor_path} index_sha256 does not match embedded audit index digest"
        )
    anchor_record_count = anchor_value.get("record_count")
    if (
        isinstance(anchor_record_count, bool)
        or not isinstance(anchor_record_count, int)
        or anchor_record_count < 0
    ):
        raise AdapterError(f"{anchor_path} record_count must be a non-negative integer")
    if anchor_record_count != audit_index.get("record_count"):
        raise AdapterError(f"{anchor_path} record_count does not match embedded audit index")
    _verify_persisted_record_sources(
        audit_index,
        _record_store_dir(anchor_value, str(anchor_path)),
        str(anchor_path),
        allow_missing_record_sources=allow_missing_record_sources,
    )

    index_file = export_dir / INDEX_FILE
    exported_index = verify_audit_index(_load_json(index_file))
    if exported_index != audit_index:
        raise AdapterError(f"{anchor_path} embedded audit index differs from {index_file}")

    anchors_dir = export_dir / ANCHOR_DIR
    try:
        relative_anchor = anchor_path.resolve().relative_to(anchors_dir.resolve())
    except ValueError:
        relative_anchor = None
    if relative_anchor is not None:
        expected_name = f"{index_sha256}.notary.json"
        if relative_anchor.name != expected_name:
            raise AdapterError(
                f"{anchor_path} filename must be digest-addressed as {expected_name}"
            )

    latest = export_dir / LATEST_ANCHOR_FILE
    if anchor_path.resolve() == latest.resolve():
        digest_anchor = anchors_dir / f"{index_sha256}.notary.json"
        if not digest_anchor.exists():
            raise AdapterError(f"{latest} has no digest-addressed peer {digest_anchor}")
        if _read_regular_file(
            digest_anchor,
            max_bytes=MAX_AUDIT_EXPORT_JSON_BYTES,
        ) != raw:
            raise AdapterError(f"{latest} differs from digest-addressed peer {digest_anchor}")

    return VerifiedAnchor(
        path=anchor_path,
        payload=anchor_value,
        raw=raw,
        index_sha256=index_sha256,
        anchor_sha256=anchor_sha256,
        record_count=anchor_record_count,
    )


def discover_anchor_paths(export_dir: Path, all_anchors: bool) -> list[Path]:
    """Return anchor paths to publish in deterministic order."""

    if all_anchors:
        anchors = sorted((export_dir / ANCHOR_DIR).glob("*.notary.json"))
        if not anchors:
            raise AdapterError(f"{export_dir / ANCHOR_DIR} has no *.notary.json anchors")
        return anchors
    return [export_dir / LATEST_ANCHOR_FILE]


def _endpoint_sha256(endpoint: str) -> str:
    return sha256_hex(endpoint.encode("utf-8"))


def _reject_url_control_chars(url: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise AdapterError(f"{label} must not contain control characters")


def _reject_url_percent_encoding_smuggling(url: str, label: str) -> None:
    index = 0
    while True:
        index = url.find("%", index)
        if index == -1:
            return
        token = url[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise AdapterError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise AdapterError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        index += 3


def _validate_url_port(parsed: urllib.parse.ParseResult, label: str) -> None:
    try:
        port = parsed.port
    except ValueError as error:
        raise AdapterError(f"{label} has invalid port") from error
    port_text = _raw_url_port_text(parsed)
    if port_text == "":
        raise AdapterError(f"{label} must not include an empty port")
    if port_text is not None:
        if len(port_text) > 1 and port_text.startswith("0"):
            raise AdapterError(f"{label} port must not contain leading zeros")
        if port == 0:
            raise AdapterError(f"{label} port must be positive")
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise AdapterError(f"{label} must not explicitly specify the default port")


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
        raise AdapterError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise AdapterError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise AdapterError(f"{label} host must not end with a dot")
    _reject_secret_looking_identifier(raw_host, f"{label} host")
    if len(raw_host) > 253:
        raise AdapterError(f"{label} host must be at most 253 characters")
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise AdapterError(f"{label} host must be a valid IP address")
    _reject_legacy_ipv4_host_notation(raw_host, label)
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise AdapterError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise AdapterError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise AdapterError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise AdapterError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise AdapterError(
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
        raise AdapterError(f"{label} host must not use legacy IPv4 numeric notation")


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
        raise AdapterError(f"{label} must not use localhost")
    if _host_uses_rebinding_suffix(hostname):
        raise AdapterError(f"{label} must not use local/private rebinding hostnames")
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        return
    if not address.is_global:
        raise AdapterError(f"{label} must not use local, private, or reserved IP addresses")
    if _address_embeds_non_global_ipv4(address):
        raise AdapterError(f"{label} must not embed local, private, or reserved IPv4 addresses")


def _host_uses_rebinding_suffix(hostname: str) -> bool:
    return hostname in LOCAL_REBINDING_HOST_SUFFIXES or any(
        hostname.endswith("." + suffix) for suffix in LOCAL_REBINDING_HOST_SUFFIXES
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


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if "\\" in path:
        raise AdapterError(f"{label} path must use forward slashes")
    if ";" in path:
        raise AdapterError(f"{label} path must not contain semicolon parameters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise AdapterError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise AdapterError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _is_secret_looking_key(path):
        raise AdapterError(f"{label} path must not contain secret-looking material")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise AdapterError(f"{label} path must not contain encoded dot or separator characters")
    if "%3b" in lowered:
        raise AdapterError(f"{label} path must not contain encoded semicolon parameters")
    if any(token in lowered for token in ("%23", "%3a", "%3f", "%40", "%5b", "%5d")):
        raise AdapterError(f"{label} path must not contain encoded URL delimiter characters")
    if "%25" in lowered:
        raise AdapterError(f"{label} path must not contain encoded percent characters")


def _validate_endpoint(endpoint: str, allow_insecure_http: bool) -> None:
    label = "endpoint"
    if len(endpoint) > MAX_HTTP_URL_CHARS:
        raise AdapterError(f"{label} must be no longer than {MAX_HTTP_URL_CHARS} characters")
    _reject_url_control_chars(endpoint, label)
    _reject_url_percent_encoding_smuggling(endpoint, label)
    if endpoint != endpoint.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in endpoint):
        raise AdapterError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(endpoint)
        hostname = parsed.hostname
    except ValueError as error:
        raise AdapterError(f"{label} is not a valid URL") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise AdapterError(
                f"refusing insecure HTTP {label}; pass --allow-insecure-http for local tests"
            )
        raise AdapterError(f"{label} must use http or https")
    _validate_url_port(parsed, label)
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise AdapterError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise AdapterError(f"{label} must not contain credentials")
    _validate_url_host(parsed, label)
    _reject_local_url_host(
        parsed,
        label,
        allow_insecure_http=allow_insecure_http,
    )
    if parsed.params or parsed.query or parsed.fragment:
        raise AdapterError(f"{label} must not contain params, query, or fragment")
    _validate_url_path(parsed, label)


def _reject_duplicate_endpoints(endpoints: list[str]) -> None:
    seen: dict[str, int] = {}
    for offset, endpoint in enumerate(endpoints):
        if endpoint in seen:
            raise AdapterError(
                f"--endpoint[{offset}] duplicates --endpoint[{seen[endpoint]}]"
            )
        seen[endpoint] = offset


def _load_bearer_token(path: Path | None) -> str | None:
    if path is None:
        return None
    label = "bearer token file"
    raw = _read_regular_file(
        path,
        max_bytes=MAX_BEARER_TOKEN_BYTES,
        limit_label="bearer token",
        path_label=label,
    )
    try:
        token = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AdapterError(f"{label} is not UTF-8") from error
    if not token:
        raise AdapterError(f"{label} is empty")
    if token != token.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in token):
        raise AdapterError(f"{label} must not contain control characters")
    if any(ch.isspace() for ch in token):
        raise AdapterError(f"{label} must not contain whitespace")
    return token


def publish_anchor(
    anchor: VerifiedAnchor,
    endpoint: str,
    *,
    timeout_secs: float,
    response_limit_bytes: int,
    bearer_token: str | None,
) -> PublishResult:
    """POST a verified anchor preimage and return a bounded outcome."""

    headers = {
        "Content-Type": "application/json",
        "X-Iroha-Iso-Anchor-Sha256": anchor.anchor_sha256,
        "X-Iroha-Iso-Index-Sha256": anchor.index_sha256,
    }
    if bearer_token is not None:
        headers["Authorization"] = f"Bearer {bearer_token}"
    request = urllib.request.Request(endpoint, data=anchor.raw, headers=headers, method="POST")
    try:
        with NO_REDIRECT_OPENER.open(request, timeout=timeout_secs) as response:
            body = response.read(response_limit_bytes + 1)
            if len(body) > response_limit_bytes:
                raise AdapterError(
                    f"endpoint response exceeded {response_limit_bytes} byte limit"
                )
            status_code = int(response.status)
    except urllib.error.HTTPError as error:
        try:
            body = error.read(response_limit_bytes + 1)
        finally:
            error.close()
        if len(body) > response_limit_bytes:
            raise AdapterError(
                f"endpoint error response exceeded {response_limit_bytes} byte limit"
            )
        return PublishResult(
            endpoint=endpoint,
            status_code=int(error.code),
            ok=False,
            response_body_sha256=sha256_hex(body),
            response_body_preview=_response_preview(body),
            error=f"HTTP {error.code}",
        )
    except urllib.error.URLError as error:
        return PublishResult(
            endpoint=endpoint,
            status_code=None,
            ok=False,
            response_body_sha256=None,
            response_body_preview=None,
            error=_receipt_error(str(error.reason)),
        )

    ok = 200 <= status_code <= 299
    return PublishResult(
        endpoint=endpoint,
        status_code=status_code,
        ok=ok,
        response_body_sha256=sha256_hex(body),
        response_body_preview=_response_preview(body),
        error=None if ok else f"HTTP {status_code}",
    )


def _response_preview(body: bytes) -> str:
    preview = body[:4096].decode("utf-8", errors="replace")
    if _response_preview_looks_secret(preview):
        return REDACTED_RESPONSE_PREVIEW
    return preview


def _response_preview_looks_secret(preview: str) -> bool:
    return any(
        marker in candidate.lower()
        for candidate in _secret_scan_values(preview)
        for marker in SECRET_PREVIEW_MARKERS
    )


def _receipt_error(message: str) -> str:
    if _response_preview_looks_secret(message):
        return REDACTED_ERROR
    return message


def receipt_value(anchor: VerifiedAnchor, result: PublishResult) -> dict[str, Any]:
    """Build a receipt JSON object for one publication attempt."""

    receipt: dict[str, Any] = {
        "version": RECEIPT_VERSION,
        "receipt_kind": "iso-audit-notary",
        "published_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "endpoint": result.endpoint,
        "endpoint_sha256": _endpoint_sha256(result.endpoint),
        "anchor_path": str(anchor.path),
        "anchor_sha256": anchor.anchor_sha256,
        "index_sha256": anchor.index_sha256,
        "record_count": anchor.record_count,
        "status_code": result.status_code,
        "ok": result.ok,
        "response_body_sha256": result.response_body_sha256,
        "response_body_preview": result.response_body_preview,
        "error": result.error,
    }
    receipt[RECEIPT_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(receipt))
    return receipt


def write_receipt(receipt_dir: Path, anchor: VerifiedAnchor, result: PublishResult) -> Path:
    """Write one receipt and return its path."""

    _ensure_output_directory(receipt_dir, "receipt_dir")
    receipt = receipt_value(anchor, result)
    path = receipt_output_path(receipt_dir, anchor, result.endpoint)
    _write_text_output(path, json.dumps(receipt, indent=2, sort_keys=False) + "\n")
    return path


def receipt_output_path(receipt_dir: Path, anchor: VerifiedAnchor, endpoint: str) -> Path:
    """Return the receipt path for one anchor and endpoint."""

    return receipt_dir / (
        f"{anchor.index_sha256}.{_endpoint_sha256(endpoint)}.receipt.json"
    )


def run(args: argparse.Namespace) -> int:
    timeout_secs = _require_positive_finite_cli_number(args.timeout_secs, "--timeout-secs")
    response_limit_bytes = _require_positive_cli_int(
        args.response_limit_bytes, "--response-limit-bytes"
    )
    _ensure_input_directory(args.export_dir, "export_dir")
    export_dir = args.export_dir
    receipt_dir = _absolute_path_without_resolving_leaf(
        args.receipt_dir or export_dir / "receipts"
    )
    endpoints = list(args.endpoint)
    for endpoint in endpoints:
        _validate_endpoint(endpoint, args.allow_insecure_http)
    _reject_duplicate_endpoints(endpoints)
    bearer_token = _load_bearer_token(args.bearer_token_file)

    anchors = [
        verify_anchor_file(
            export_dir,
            anchor_path,
            allow_missing_record_sources=args.allow_missing_record_sources,
        )
        for anchor_path in discover_anchor_paths(export_dir, args.all)
    ]
    if args.dry_run:
        summary = {
            "validated_anchors": len(anchors),
            "index_sha256": [anchor.index_sha256 for anchor in anchors],
            "record_count": [anchor.record_count for anchor in anchors],
            "dry_run": True,
        }
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0
    if not endpoints:
        raise AdapterError("at least one --endpoint is required unless --dry-run is set")
    _ensure_output_directory(receipt_dir, "receipt_dir")
    for anchor in anchors:
        for endpoint in endpoints:
            _ensure_output_file_target(receipt_output_path(receipt_dir, anchor, endpoint))

    failures = 0
    receipts: list[str] = []
    for anchor in anchors:
        for endpoint in endpoints:
            result = publish_anchor(
                anchor,
                endpoint,
                timeout_secs=timeout_secs,
                response_limit_bytes=response_limit_bytes,
                bearer_token=bearer_token,
            )
            receipts.append(str(write_receipt(receipt_dir, anchor, result)))
            if not result.ok:
                failures += 1

    summary = {
        "published_anchors": len(anchors),
        "endpoint_count": len(endpoints),
        "receipts": receipts,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failures else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify and publish ISO 20022 audit_export_dir notary anchors."
    )
    parser.add_argument(
        "--export-dir",
        required=True,
        type=Path,
        help="Torii iso_bridge.audit_export_dir containing messages.index.json and anchors/.",
    )
    parser.add_argument(
        "--endpoint",
        action="append",
        default=[],
        help="HTTPS archival/notary endpoint to POST each verified anchor to; repeatable.",
    )
    parser.add_argument(
        "--receipt-dir",
        type=Path,
        help="Directory for local publication receipts (default: <export-dir>/receipts).",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Publish all digest-addressed anchors instead of latest.notary.json.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Only verify preimages and print a validation summary; do not publish.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// endpoints for local tests; production endpoints should use HTTPS.",
    )
    parser.add_argument(
        "--allow-missing-record-sources",
        action="store_true",
        help=(
            "Allow non-empty notary anchors without local store_dir/messages record "
            "sources for local anchor-only diagnostics; production publication "
            "should keep the default fail-closed behavior."
        ),
    )
    parser.add_argument(
        "--bearer-token-file",
        type=Path,
        help="Runtime-only file containing a bearer token for endpoint Authorization.",
    )
    parser.add_argument(
        "--timeout-secs",
        type=float,
        default=10.0,
        help="HTTP timeout in seconds per publication attempt.",
    )
    parser.add_argument(
        "--response-limit-bytes",
        type=int,
        default=DEFAULT_RESPONSE_LIMIT_BYTES,
        help="Maximum response body bytes retained in a receipt.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        _preflight_raw_cli_secrets(
            argv,
            {
                "--bearer-token-file",
                "--endpoint",
                "--export-dir",
                "--receipt-dir",
                "--response-limit-bytes",
                "--timeout-secs",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--all",
                "--allow-insecure-http",
                "--allow-missing-record-sources",
                "--dry-run",
            },
        )
        _preflight_required_cli_values(argv, {"--endpoint"}, "URL")
        _preflight_numeric_cli_values(
            argv,
            integer_flags={"--response-limit-bytes"},
            number_flags={"--timeout-secs"},
        )
        _preflight_output_cli_paths(
            argv,
            {"--bearer-token-file", "--export-dir", "--receipt-dir"},
        )
        args = parser.parse_args(argv)
        return run(args)
    except AdapterError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
