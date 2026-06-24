#!/usr/bin/env python3
"""Submit operator rail-gateway ISO 20022 file drops to Torii.

Purpose:
  This operator-side adapter bridges a live rail gateway drop directory into
  Torii's ISO 20022 REST endpoints. Each ``*.xml`` payload must have a sidecar
  ``*.json`` file that pins ``message_type``, ``profile``, and
  ``payload_sha256``. The adapter verifies those fields before network
  submission and writes a bounded local receipt for every submit attempt.

Prerequisites:
  Python 3.11+ and a Torii endpoint with the ISO bridge enabled. No third party
  Python packages are required.

Safety:
  The script never deletes input files. Plain HTTP Torii URLs are rejected unless
  ``--allow-insecure-http`` is supplied for local tests. Bearer tokens are read
  from a runtime-only file and are never persisted into receipts.
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
import unicodedata
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_MAX_PAYLOAD_BYTES = 4 * 1024 * 1024
DEFAULT_RESPONSE_LIMIT_BYTES = 64 * 1024
MAX_BEARER_TOKEN_BYTES = 8192
MAX_HTTP_URL_CHARS = 2048
MAX_LOCAL_PATH_CHARS = 4096
MAX_SIDECAR_JSON_BYTES = 16 * 1024
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
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
LEGACY_MESSAGE_TYPES = {"colr.007"}
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.[0-9]{3}$")
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MAX_RAIL_MESSAGE_ID_CHARS = 128
RAIL_MESSAGE_ID_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9._:@+-]*[A-Za-z0-9])?$")
SIDECAR_KEYS = {"message_type", "profile", "payload_sha256", "rail_message_id"}
SECRET_PREVIEW_MARKERS = (
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
SECRET_VALUE_PATTERNS = [
    re.compile(r"\bauthorization\s*:", re.IGNORECASE),
    re.compile(r"\bbearer\s+[A-Za-z0-9._~+/=-]+", re.IGNORECASE),
    re.compile(
        r"\b(?:token|secret|private[\s_./\\-]*key|password|passphrase|api[\s_./\\-]*key|access[\s_./\\-]*key|session[\s_./\\-]*key|client[\s_./\\-]*secret|cookie|set[\s_./\\-]*cookie)\s*[:=]\s*\S+",
        re.IGNORECASE,
    ),
    re.compile(r"\bx[\s_./\\-]*iroha[\s_./\\-]*signature\s*:", re.IGNORECASE),
]
CLI_OPTION_FLAGS = {
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
}
REDACTED_RESPONSE_PREVIEW = "[redacted: sensitive response body]"
REDACTED_ERROR = "[redacted: sensitive error]"

ENDPOINTS = {
    "pacs.008": "pacs008",
    "pacs.009": "pacs009",
    "pacs.002": "pacs002",
    "pacs.004": "pacs004",
    "camt.056": "camt056",
    "sese.023": "sese023",
    "sese.024": "sese024",
    "sese.025": "sese025",
    "colr.007": "colr007",
    "colr.012": "colr012",
}


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


def _check_no_secret_material(value: Any, label: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if _is_secret_looking_key(str(key)):
                raise AdapterError(f"{label} contains forbidden secret-looking field")
            if _is_control_bearing_key(key):
                raise AdapterError(f"{label} contains forbidden control-bearing field")
            _check_no_secret_material(child, f"{label}.{key}")
    elif isinstance(value, list):
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{label}[{offset}]")
    elif isinstance(value, str):
        if _contains_unsafe_json_control(value):
            raise AdapterError(f"{label} contains unsafe control characters")
        if _contains_secret_material(value):
            raise AdapterError(f"{label} contains secret-looking material")


NO_REDIRECT_OPENER = urllib.request.build_opener(_NoRedirectHandler)


class AdapterError(RuntimeError):
    """Raised when an inbound rail file cannot be safely submitted."""


@dataclass(frozen=True)
class GatewayMessage:
    """Verified file-drop message ready for Torii submission."""

    xml_path: Path
    sidecar_path: Path
    payload: bytes
    payload_sha256: str
    message_type: str
    profile: str | None
    rail_message_id: str | None


@dataclass(frozen=True)
class SubmitResult:
    """Torii submission result for one message."""

    status_code: int | None
    ok: bool
    response_body_sha256: str | None
    response_body_preview: str | None
    error: str | None = None


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _is_lower_hex_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _is_all_zero_sha256(value: str) -> bool:
    return all(ch == "0" for ch in value)


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


def _validate_rail_message_id(value: str, label: str) -> None:
    if len(value) > MAX_RAIL_MESSAGE_ID_CHARS:
        raise AdapterError(f"{label} must be at most {MAX_RAIL_MESSAGE_ID_CHARS} characters")
    if RAIL_MESSAGE_ID_RE.fullmatch(value) is None:
        raise AdapterError(f"{label} must be a canonical ASCII rail message id")


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
        raise AdapterError(f"{label} must not contain secret-looking material")


def _reject_non_ascii_identifier(value: str, label: str) -> None:
    if any(ord(ch) > 0x7E for ch in value):
        raise AdapterError(f"{label} must use printable ASCII")


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    if set(value) - allowed:
        raise AdapterError(f"{label} contains unknown keys")


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise AdapterError("JSON object contains duplicate key")
        seen.add(key)
        result[key] = value
    return result


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
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise AdapterError(f"{display_path} must be a regular file")
        if max_bytes is not None and opened.st_size > max_bytes:
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
    _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{label} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise AdapterError(f"{label} must be a directory")


def _reject_symlinked_existing_ancestors(
    path: Path, *, display_label: str | None = None
) -> None:
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
            label = display_label if display_label is not None else str(current)
            raise AdapterError(f"{label} must not be a symlink")


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise AdapterError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise AdapterError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise AdapterError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise AdapterError(f"{label} must not contain encoded semicolon parameters")
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise AdapterError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise AdapterError(f"{label} must not contain encoded percent characters")
        index += 3


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise AdapterError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise AdapterError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
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
    if ":" in raw:
        raise AdapterError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise AdapterError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise AdapterError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise AdapterError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise AdapterError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise AdapterError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
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
    if ":" in raw:
        raise AdapterError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise AdapterError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
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
        if arg == "--":
            raise AdapterError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if _contains_control_character(arg):
            raise AdapterError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise AdapterError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise AdapterError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise AdapterError("argument terminator is not supported")
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
            raise AdapterError("argument terminator is not supported")
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
                if not value or value in CLI_OPTION_FLAGS:
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
            raise AdapterError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise AdapterError(f"{flag} requires a {value_name} value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a {value_name} value")
                if value_name == "URL":
                    _reject_raw_url_cli_value(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise AdapterError(f"{flag} requires a {value_name} value")
                if value_name == "URL":
                    _reject_raw_url_cli_value(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_url_cli_value(raw: str, flag: str) -> None:
    if _contains_control_character(raw):
        raise AdapterError(f"{flag} URL must not contain control characters")
    if raw != raw.strip():
        raise AdapterError(f"{flag} URL must not have surrounding whitespace")
    if any(ord(ch) > 0x7E for ch in raw):
        raise AdapterError(f"{flag} URL must use printable ASCII")
    if not raw.lower().startswith(("http://", "https://")) and (
        _contains_secret_material(raw) or _contains_secret_identifier_material(raw)
    ):
        raise AdapterError(f"{flag} URL must not contain secret-looking material")


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or _contains_control_character(raw):
        raise AdapterError(f"{flag} must be a numeric value")
    if any(ord(ch) > 0x7E for ch in raw):
        raise AdapterError(f"{flag} must use printable ASCII")
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
            raise AdapterError("argument terminator is not supported")
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
    if _path_is_repository_iso_fixture(str(path)):
        raise AdapterError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )
    _reject_symlinked_existing_ancestors(path, display_label=label)
    if path.exists() or path.is_symlink():
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise AdapterError(f"{label} must not be a symlink")
        if not stat.S_ISDIR(mode):
            raise AdapterError(f"{label} must be a directory")
        return
    path.mkdir(parents=True, exist_ok=True)
    mode = path.lstat().st_mode
    if stat.S_ISLNK(mode):
        raise AdapterError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(mode):
        raise AdapterError(f"{label} must be a directory")


def _ensure_output_file_target(path: Path, *, display_label: str | None = None) -> None:
    label = display_label if display_label is not None else str(path)
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise AdapterError(f"{label} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise AdapterError(f"{label} must be a regular file")
        if metadata.st_nlink > 1:
            raise AdapterError(f"{label} must not be hard-linked")


def _write_text_output(path: Path, text: str, *, display_label: str | None = None) -> None:
    label = display_label if display_label is not None else "output path"
    _reject_output_path_smuggling(path, label)
    if _path_is_repository_iso_fixture(str(path)):
        raise AdapterError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )
    _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except FileExistsError as error:
        raise AdapterError(f"{label} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise AdapterError(f"{label} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise AdapterError(f"{label} must be a directory")
    _ensure_output_file_target(path, display_label=label)
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise AdapterError(f"{label} must not be a symlink") from error
        raise AdapterError(f"{label} must be a directory") from error

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
                raise AdapterError(f"{label} temp file must not be a symlink") from error
            raise AdapterError(
                f"cannot open temporary output for {label}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise AdapterError(f"{label} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise AdapterError(f"{label} temp file must not be hard-linked")
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


def _load_json(
    path: Path,
    *,
    max_bytes: int | None = None,
    display_label: str | None = None,
) -> Any:
    label = display_label if display_label is not None else str(path)
    raw = (
        _bounded_read(path, max_bytes, path_label=display_label)
        if max_bytes is not None
        else _read_regular_file(path, path_label=display_label)
    )
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AdapterError(f"{label} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise AdapterError(f"{label} is not valid JSON: {error}") from error
    _reject_json_surrogates(value)
    return value


def _reject_json_constant(value: str) -> None:
    raise AdapterError("JSON contains non-finite numeric constant")


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


def _bounded_read(
    path: Path,
    max_bytes: int,
    *,
    path_label: str | None = None,
) -> bytes:
    if isinstance(max_bytes, bool) or not isinstance(max_bytes, int) or max_bytes <= 0:
        raise AdapterError("max payload bytes must be a positive integer")
    display_path = path_label if path_label is not None else str(path)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{display_path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{display_path} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise AdapterError(f"{display_path} must be a regular file")
    size = metadata.st_size
    if size <= 0:
        raise AdapterError(f"{display_path} is empty")
    if size > max_bytes:
        raise AdapterError(f"{display_path} exceeds {max_bytes} byte payload limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise AdapterError(f"{display_path} must be a regular file")
        size = opened.st_size
        if size <= 0:
            raise AdapterError(f"{display_path} is empty")
        if size > max_bytes:
            raise AdapterError(f"{display_path} exceeds {max_bytes} byte payload limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            raw = handle.read(max_bytes + 1)
        if len(raw) > max_bytes:
            raise AdapterError(f"{display_path} exceeds {max_bytes} byte payload limit")
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


def verify_message_file(
    xml_path: Path,
    *,
    max_payload_bytes: int,
    allow_default_profile: bool,
    allow_legacy_colr007: bool,
) -> GatewayMessage:
    """Verify a gateway XML payload and its sidecar metadata."""

    xml_label = "message XML payload"
    sidecar_label = "message sidecar"
    _reject_raw_output_path_smuggling(str(xml_path), "message XML path")
    if _path_is_repository_iso_fixture(str(xml_path)):
        raise AdapterError(
            f"{xml_label} must not point to checked-in ISO XML fixtures"
        )
    _validate_path_argument(str(xml_path.name), "message XML filename")
    if xml_path.suffix.lower() != ".xml":
        raise AdapterError("message XML path must use a .xml suffix")
    sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
    _reject_raw_output_path_smuggling(str(sidecar_path), "message sidecar path")
    sidecar = _load_json(
        sidecar_path,
        max_bytes=MAX_SIDECAR_JSON_BYTES,
        display_label=sidecar_label,
    )
    if not isinstance(sidecar, dict):
        raise AdapterError(f"{sidecar_label} must contain a JSON object")
    _reject_unknown_keys(sidecar, SIDECAR_KEYS, sidecar_label)
    _check_no_secret_material(sidecar, sidecar_label)

    payload = _bounded_read(xml_path, max_payload_bytes, path_label=xml_label)
    actual_sha256 = sha256_hex(payload)
    expected_sha256 = sidecar.get("payload_sha256")
    if isinstance(expected_sha256, str):
        _reject_secret_looking_identifier(
            expected_sha256,
            f"{sidecar_label} payload_sha256",
        )
    if not _is_lower_hex_sha256(expected_sha256):
        raise AdapterError(f"{sidecar_label} payload_sha256 must be lowercase SHA-256 hex")
    if _is_all_zero_sha256(expected_sha256):
        raise AdapterError(f"{sidecar_label} payload_sha256 must not be all zero")
    if expected_sha256 != actual_sha256:
        raise AdapterError(f"{xml_label} payload_sha256 mismatch")

    message_type = sidecar.get("message_type")
    if isinstance(message_type, str):
        _reject_non_ascii_identifier(
            message_type,
            f"{sidecar_label} message_type",
        )
        _reject_secret_looking_identifier(
            message_type,
            f"{sidecar_label} message_type",
        )
    if not isinstance(message_type, str) or MESSAGE_TYPE_RE.fullmatch(message_type) is None:
        raise AdapterError(f"{sidecar_label} message_type must be lowercase ISO family id")
    if message_type not in ENDPOINTS:
        raise AdapterError(f"{sidecar_label} has unsupported message_type")
    if message_type in LEGACY_MESSAGE_TYPES and not allow_legacy_colr007:
        raise AdapterError(
            f"{sidecar_label} uses legacy message_type; "
            "use colr.012 for production collateral substitution confirmations"
        )

    profile_present = "profile" in sidecar
    if not profile_present:
        if not allow_default_profile:
            raise AdapterError(f"{sidecar_label} must specify profile for live rail submission")
        profile = None
    else:
        profile = sidecar.get("profile")
    if profile_present and (not isinstance(profile, str) or not profile.strip()):
        raise AdapterError(f"{sidecar_label} profile must be a non-empty string")
    if isinstance(profile, str):
        if _contains_control_character(profile):
            raise AdapterError(f"{sidecar_label} profile must not contain control characters")
        if profile != profile.strip():
            raise AdapterError(
                f"{sidecar_label} profile must not have surrounding whitespace"
            )
        if any(ch.isspace() for ch in profile):
            raise AdapterError(f"{sidecar_label} profile must not contain whitespace")
        if PROFILE_ID_RE.fullmatch(profile) is None:
            raise AdapterError(
                f"{sidecar_label} profile must be a canonical lowercase profile id"
            )
        _reject_secret_looking_identifier(profile, f"{sidecar_label} profile")

    rail_message_id_present = "rail_message_id" in sidecar
    rail_message_id = None
    if rail_message_id_present:
        rail_message_id = sidecar.get("rail_message_id")
    if rail_message_id_present and (
        not isinstance(rail_message_id, str) or not rail_message_id.strip()
    ):
        raise AdapterError(f"{sidecar_label} rail_message_id must be a non-empty string")
    if isinstance(rail_message_id, str):
        if _contains_control_character(rail_message_id):
            raise AdapterError(
                f"{sidecar_label} rail_message_id must not contain control characters"
            )
        if rail_message_id != rail_message_id.strip():
            raise AdapterError(
                f"{sidecar_label} rail_message_id must not have surrounding whitespace"
            )
        if any(ch.isspace() for ch in rail_message_id):
            raise AdapterError(
                f"{sidecar_label} rail_message_id must not contain whitespace"
            )
        _validate_rail_message_id(rail_message_id, f"{sidecar_label} rail_message_id")
        _reject_secret_looking_identifier(
            rail_message_id,
            f"{sidecar_label} rail_message_id",
        )

    return GatewayMessage(
        xml_path=xml_path,
        sidecar_path=sidecar_path,
        payload=payload,
        payload_sha256=actual_sha256,
        message_type=message_type,
        profile=profile,
        rail_message_id=rail_message_id if isinstance(rail_message_id, str) else None,
    )


def discover_messages(inbox_dir: Path, *, display_label: str | None = None) -> list[Path]:
    """Return inbound XML paths in deterministic order."""

    label = display_label or "inbox_dir"
    if not inbox_dir.is_dir():
        raise AdapterError(f"{label} is not a directory")
    messages = sorted(path for path in inbox_dir.iterdir() if path.suffix.lower() == ".xml")
    if not messages:
        raise AdapterError(f"{label} has no *.xml gateway messages")
    for path in messages:
        _validate_path_argument(str(path.name), f"{label} filename")
    return messages


def _validate_path_argument(raw: str, label: str) -> None:
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise AdapterError(f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters")
    if _contains_control_character(raw):
        raise AdapterError(f"{label} must not contain control characters")
    if any(ch.isspace() for ch in raw):
        raise AdapterError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise AdapterError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise AdapterError(f"{label} must use forward slashes")
    if ";" in raw:
        raise AdapterError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise AdapterError(f"{label} must not contain URI or drive prefixes")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    for offset, part in enumerate(parts):
        if part == "" and offset != 0:
            raise AdapterError(f"{label} must not contain empty path segments")
        if part.startswith("-"):
            raise AdapterError(f"{label} must not contain leading-dash path segments")
        if part in {".", ".."}:
            raise AdapterError(f"{label} must not contain dot or parent segments")


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


def resolve_message_paths(inbox_dir: Path, message: str | None) -> list[Path]:
    """Resolve one explicit message or discover all messages under the inbox."""

    _ensure_input_directory(inbox_dir, "inbox_dir")
    inbox_root = inbox_dir.resolve()
    if message is None:
        return discover_messages(inbox_dir, display_label="inbox_dir")
    _validate_path_argument(message, "--message path")
    raw_message = Path(message).expanduser()
    message_path = raw_message if raw_message.is_absolute() else inbox_dir / raw_message
    resolved_parent = message_path.parent.resolve()
    if not resolved_parent.is_relative_to(inbox_root):
        raise AdapterError("--message path must stay under --inbox-dir")
    return [resolved_parent / message_path.name]


def _normalise_message_argument(value: Any) -> str | None:
    if value is None:
        return None
    if isinstance(value, (str, os.PathLike)):
        raw = os.fspath(value)
        if raw == "":
            raise AdapterError("message must be a non-empty path")
        return raw
    if isinstance(value, (list, tuple)):
        if not value:
            return None
        if len(value) != 1:
            raise AdapterError("provide at most one --message")
        return _normalise_message_argument(value[0])
    raise AdapterError("message must be a path")


def _reject_url_control_chars(url: str, label: str) -> None:
    if _contains_control_character(url):
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
    if any(ord(ch) > 0x7E for ch in raw_host):
        raise AdapterError(f"{label} host must use printable ASCII")
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
        raise AdapterError(f"{label} must not use reserved placeholder hostnames")


def _reject_template_canary_url_host(
    parsed: urllib.parse.ParseResult,
    label: str,
) -> None:
    hostname = (parsed.hostname or "").strip().lower()
    if _host_uses_template_canary_suffix(hostname):
        raise AdapterError(f"{label} must not use template canary hostnames")


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
        raise AdapterError(f"{label} path must use printable ASCII")
    if "\\" in path:
        raise AdapterError(f"{label} path must use forward slashes")
    if ";" in path:
        raise AdapterError(f"{label} path must not contain semicolon parameters")
    if any(token in path for token in (":", "@", "[", "]")):
        raise AdapterError(f"{label} path must not contain URL delimiter characters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise AdapterError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise AdapterError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
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
    if re.search(r"%[89a-f][0-9a-f]", lowered):
        raise AdapterError(f"{label} path must not contain percent-encoded non-ASCII bytes")


def _validate_base_url(base_url: str, allow_insecure_http: bool) -> str:
    label = "Torii URL"
    if len(base_url) > MAX_HTTP_URL_CHARS:
        raise AdapterError(f"{label} must be no longer than {MAX_HTTP_URL_CHARS} characters")
    _reject_url_control_chars(base_url, label)
    _reject_url_percent_encoding_smuggling(base_url, label)
    if base_url != base_url.strip():
        raise AdapterError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in base_url):
        raise AdapterError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(base_url)
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
    _reject_reserved_placeholder_url_host(parsed, label)
    _reject_template_canary_url_host(parsed, label)
    return base_url.rstrip("/")


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
    if _contains_control_character(token):
        raise AdapterError(f"{label} must not contain control characters")
    if any(ch.isspace() for ch in token):
        raise AdapterError(f"{label} must not contain whitespace")
    return token


def torii_url(base_url: str, message: GatewayMessage) -> str:
    """Build the Torii ISO endpoint URL for a verified message."""

    endpoint = ENDPOINTS.get(message.message_type)
    if endpoint is None:
        raise AdapterError("unsupported message_type")
    return f"{base_url}/v1/iso20022/{endpoint}"


def submit_message(
    base_url: str,
    message: GatewayMessage,
    *,
    timeout_secs: float,
    response_limit_bytes: int,
    bearer_token: str | None,
) -> SubmitResult:
    """Submit one verified message to Torii."""

    headers = {
        "Content-Type": "application/xml",
        "X-Iroha-Iso-Gateway-Payload-Sha256": message.payload_sha256,
    }
    if message.profile is not None:
        headers["X-Iroha-Iso-Profile"] = message.profile
    if message.rail_message_id is not None:
        headers["X-Iroha-Iso-Rail-Message-Id"] = message.rail_message_id
    if bearer_token is not None:
        headers["Authorization"] = f"Bearer {bearer_token}"

    request = urllib.request.Request(
        torii_url(base_url, message),
        data=message.payload,
        headers=headers,
        method="POST",
    )
    try:
        with NO_REDIRECT_OPENER.open(request, timeout=timeout_secs) as response:
            status_code = int(response.status)
            if not _is_http_status_code(status_code):
                return _invalid_http_status_result(status_code)
            body = response.read(response_limit_bytes + 1)
            if len(body) > response_limit_bytes:
                raise AdapterError(
                    f"Torii response exceeded {response_limit_bytes} byte limit"
                )
            if 200 <= status_code <= 299 and _response_body_looks_secret(body):
                raise AdapterError("Torii response body contains secret-looking material")
            if 200 <= status_code <= 299 and _response_body_has_unsafe_control(body):
                raise AdapterError("Torii response body contains unsafe control characters")
    except urllib.error.HTTPError as error:
        status_code = int(error.code)
        if not _is_http_status_code(status_code):
            error.close()
            return _invalid_http_status_result(status_code)
        try:
            body = error.read(response_limit_bytes + 1)
        finally:
            error.close()
        if len(body) > response_limit_bytes:
            raise AdapterError(f"Torii error response exceeded {response_limit_bytes} byte limit")
        return SubmitResult(
            status_code=status_code,
            ok=False,
            response_body_sha256=sha256_hex(body),
            response_body_preview=_response_preview(body),
            error=f"HTTP {status_code}",
        )
    except urllib.error.URLError as error:
        return SubmitResult(
            status_code=None,
            ok=False,
            response_body_sha256=None,
            response_body_preview=None,
            error=_receipt_error(str(error.reason)),
        )

    ok = 200 <= status_code <= 299
    return SubmitResult(
        status_code=status_code,
        ok=ok,
        response_body_sha256=sha256_hex(body),
        response_body_preview=_response_preview(body),
        error=None if ok else f"HTTP {status_code}",
    )


def _is_http_status_code(status_code: int) -> bool:
    return 100 <= status_code <= 599


def _invalid_http_status_result(status_code: int) -> SubmitResult:
    return SubmitResult(
        status_code=None,
        ok=False,
        response_body_sha256=None,
        response_body_preview=None,
        error=f"invalid HTTP status {status_code}",
    )


def _response_preview(body: bytes) -> str:
    preview = body[:4096].decode("utf-8", errors="replace")
    if _response_preview_looks_secret(preview) or _contains_unsafe_preview_control(
        preview
    ):
        return REDACTED_RESPONSE_PREVIEW
    return preview


def _response_body_looks_secret(body: bytes) -> bool:
    return _response_preview_looks_secret(body.decode("utf-8", errors="replace"))


def _response_body_has_unsafe_control(body: bytes) -> bool:
    return _contains_unsafe_preview_control(body.decode("utf-8", errors="replace"))


def _response_preview_looks_secret(preview: str) -> bool:
    return _contains_secret_material(preview) or any(
        _contains_secret_marker(candidate, SECRET_PREVIEW_MARKERS)
        for candidate in _secret_scan_values(preview)
    )


def _receipt_error(message: str) -> str:
    if _response_preview_looks_secret(message) or _contains_unsafe_preview_control(
        message
    ):
        return REDACTED_ERROR
    return message


def _contains_unsafe_preview_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def receipt_value(message: GatewayMessage, result: SubmitResult, endpoint_url: str) -> dict[str, Any]:
    """Build a receipt JSON object for one Torii submission attempt."""

    receipt: dict[str, Any] = {
        "version": RECEIPT_VERSION,
        "receipt_kind": "iso-rail-gateway",
        "submitted_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "xml_path": str(message.xml_path),
        "sidecar_path": str(message.sidecar_path),
        "message_type": message.message_type,
        "profile": message.profile,
        "rail_message_id": message.rail_message_id,
        "payload_sha256": message.payload_sha256,
        "endpoint_url": endpoint_url,
        "endpoint_sha256": sha256_hex(endpoint_url.encode("utf-8")),
        "status_code": result.status_code,
        "ok": result.ok,
        "response_body_sha256": result.response_body_sha256,
        "response_body_preview": result.response_body_preview,
        "error": result.error,
    }
    receipt[RECEIPT_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(receipt))
    return receipt


def write_receipt(receipt_dir: Path, message: GatewayMessage, result: SubmitResult, endpoint_url: str) -> Path:
    """Write one submission receipt and return its path."""

    _ensure_output_directory(receipt_dir, "receipt_dir")
    receipt = receipt_value(message, result, endpoint_url)
    path = receipt_output_path(receipt_dir, message)
    _write_text_output(
        path,
        json.dumps(receipt, indent=2) + "\n",
        display_label="receipt output",
    )
    return path


def receipt_output_path(receipt_dir: Path, message: GatewayMessage) -> Path:
    """Return the receipt path for one gateway message."""

    return receipt_dir / f"{message.payload_sha256}.receipt.json"


def _reject_duplicate_gateway_messages(messages: list[GatewayMessage]) -> None:
    seen_payloads: dict[str, int] = {}
    seen_rail_ids: dict[str, int] = {}
    for offset, message in enumerate(messages):
        payload_sha256 = message.payload_sha256
        if payload_sha256 in seen_payloads:
            raise AdapterError(
                f"messages[{offset}].payload_sha256 duplicates "
                f"messages[{seen_payloads[payload_sha256]}].payload_sha256"
            )
        seen_payloads[payload_sha256] = offset
        if message.rail_message_id is None:
            continue
        rail_message_id = message.rail_message_id
        if rail_message_id in seen_rail_ids:
            raise AdapterError(
                f"messages[{offset}].rail_message_id duplicates "
                f"messages[{seen_rail_ids[rail_message_id]}].rail_message_id"
            )
        seen_rail_ids[rail_message_id] = offset


def _reject_unused_local_overrides(
    args: argparse.Namespace,
    *,
    base_url: str,
    messages: list[GatewayMessage],
) -> None:
    if args.allow_insecure_http:
        parsed = urllib.parse.urlparse(base_url)
        if not _url_requires_insecure_http_override(parsed):
            raise AdapterError(
                "--allow-insecure-http requires an http:// or local/private Torii URL"
            )
    if args.allow_default_profile and not any(message.profile is None for message in messages):
        raise AdapterError(
            "--allow-default-profile requires at least one sidecar without profile"
        )
    if args.allow_legacy_colr007 and not any(
        message.message_type in LEGACY_MESSAGE_TYPES for message in messages
    ):
        raise AdapterError(
            "--allow-legacy-colr007 requires at least one legacy colr.007 message"
        )


def run(args: argparse.Namespace) -> int:
    if args.inbox_dir is None:
        raise AdapterError("provide --inbox-dir")
    message = _normalise_message_argument(args.message)
    _reject_output_path_smuggling(args.inbox_dir, "inbox_dir")
    if message is not None:
        _reject_raw_output_path_smuggling(message, "message")
    if args.bearer_token_file is not None:
        _reject_output_path_smuggling(args.bearer_token_file, "bearer_token_file")
    receipt_dir_source = args.receipt_dir or args.inbox_dir / "receipts"
    _reject_output_path_smuggling(receipt_dir_source, "receipt_dir")
    receipt_dir = _absolute_path_without_resolving_leaf(
        receipt_dir_source
    )
    if _path_is_repository_iso_fixture(str(receipt_dir)):
        raise AdapterError(
            "receipt_dir must not point to checked-in ISO fixture artifacts"
        )
    if _path_is_repository_iso_fixture(str(args.inbox_dir)):
        raise AdapterError(
            "inbox_dir must not point to checked-in ISO fixture artifacts"
        )
    timeout_secs = _require_positive_finite_cli_number(args.timeout_secs, "--timeout-secs")
    response_limit_bytes = _require_positive_cli_int(
        args.response_limit_bytes, "--response-limit-bytes"
    )
    max_payload_bytes = _require_positive_cli_int(
        args.max_payload_bytes, "--max-payload-bytes"
    )
    base_url = _validate_base_url(args.torii_base_url, args.allow_insecure_http)
    _ensure_input_directory(args.inbox_dir, "inbox_dir")
    inbox_dir = args.inbox_dir
    bearer_token = _load_bearer_token(args.bearer_token_file)
    paths = resolve_message_paths(inbox_dir, message)
    messages = [
        verify_message_file(
            path,
            max_payload_bytes=max_payload_bytes,
            allow_default_profile=args.allow_default_profile,
            allow_legacy_colr007=args.allow_legacy_colr007,
        )
        for path in paths
    ]
    _reject_duplicate_gateway_messages(messages)
    _reject_unused_local_overrides(args, base_url=base_url, messages=messages)

    if args.dry_run:
        summary = {
            "dry_run": True,
            "validated_messages": len(messages),
            "payload_sha256": [message.payload_sha256 for message in messages],
            "message_type": [message.message_type for message in messages],
        }
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0

    _ensure_output_directory(receipt_dir, "receipt_dir")
    for offset, message in enumerate(messages):
        _ensure_output_file_target(
            receipt_output_path(receipt_dir, message),
            display_label=f"receipt_output[{offset}]",
        )

    failures = 0
    receipts: list[str] = []
    for message in messages:
        endpoint_url = torii_url(base_url, message)
        result = submit_message(
            base_url,
            message,
            timeout_secs=timeout_secs,
            response_limit_bytes=response_limit_bytes,
            bearer_token=bearer_token,
        )
        receipts.append(str(write_receipt(receipt_dir, message, result, endpoint_url)))
        if not result.ok:
            failures += 1

    summary = {
        "submitted_messages": len(messages),
        "receipts": receipts,
        "failures": failures,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 1 if failures else 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 rail-gateway file drops and submit them to Torii.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--inbox-dir",
        required=True,
        type=Path,
        help="Directory containing *.xml payloads and sibling *.xml.json sidecars.",
    )
    parser.add_argument(
        "--message",
        help="Submit one XML file instead of scanning --inbox-dir.",
    )
    parser.add_argument(
        "--torii-base-url",
        required=True,
        help="Torii base URL, for example https://torii.example.internal.",
    )
    parser.add_argument(
        "--receipt-dir",
        type=Path,
        help="Directory for local submission receipts (default: <inbox-dir>/receipts).",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Only verify sidecars and payload digests; do not submit.",
    )
    parser.add_argument(
        "--allow-default-profile",
        action="store_true",
        help="Allow sidecars without profile and let Torii use its default profile.",
    )
    parser.add_argument(
        "--allow-legacy-colr007",
        action="store_true",
        help="Allow legacy local colr.007 collateral-substitution file drops; production should use colr.012.",
    )
    parser.add_argument(
        "--allow-insecure-http",
        action="store_true",
        help="Allow http:// Torii URLs for local tests; production should use HTTPS.",
    )
    parser.add_argument(
        "--bearer-token-file",
        type=Path,
        help="Runtime-only file containing a bearer token for Torii Authorization.",
    )
    parser.add_argument(
        "--max-payload-bytes",
        type=int,
        default=DEFAULT_MAX_PAYLOAD_BYTES,
        help="Maximum XML payload size accepted from the gateway drop.",
    )
    parser.add_argument(
        "--timeout-secs",
        type=float,
        default=10.0,
        help="HTTP timeout in seconds per Torii submission.",
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
                "--inbox-dir",
                "--max-payload-bytes",
                "--message",
                "--receipt-dir",
                "--response-limit-bytes",
                "--timeout-secs",
                "--torii-base-url",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--allow-default-profile",
                "--allow-insecure-http",
                "--allow-legacy-colr007",
                "--dry-run",
            },
        )
        _preflight_required_cli_values(argv, {"--torii-base-url"}, "URL")
        _preflight_numeric_cli_values(
            argv,
            integer_flags={"--max-payload-bytes", "--response-limit-bytes"},
            number_flags={"--timeout-secs"},
        )
        _preflight_output_cli_paths(
            argv,
            {"--bearer-token-file", "--inbox-dir", "--message", "--receipt-dir"},
        )
        args = parser.parse_args(argv)
        return run(args)
    except AdapterError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
