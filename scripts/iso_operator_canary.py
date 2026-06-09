#!/usr/bin/env python3
"""Run a provider ISO 20022 operator canary from a checked JSON runbook.

Purpose:
  This operator-side runner ties together the live rail file-drop adapter,
  audit-notary adapter, and receipt verifier. It executes the same CLI scripts
  operators run manually, captures bounded stage output, and emits a single
  JSON summary that can be archived by CI or an operations runbook.

Prerequisites:
  Python 3.11+. No third party Python packages are required. The configured
  rail inbox, audit export directory, endpoints, and optional bearer-token files
  must already exist.

Safety:
  The runner never deletes inputs and never mutates repository files unless
  ``--summary-out`` points at a file to write. Plain HTTP remains disabled by
  default in the underlying adapters and verifier. Bearer-token file paths are
  passed through to child scripts, but token contents are never read or persisted
  by this runner.
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
import subprocess
import sys
import threading
import urllib.parse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_OUTPUT_LIMIT_BYTES = 64 * 1024
DEFAULT_STAGE_TIMEOUT_SECS = 300.0
CANARY_SUMMARY_VERSION = 1
MAX_CONFIG_JSON_BYTES = 64 * 1024
MAX_HTTP_URL_CHARS = 2048
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
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
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
SAFE_OUTPUT_CONTROL_CHARS = {"\t", "\n", "\r"}
SCRIPT_DIR = Path(__file__).resolve().parent


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

TOP_LEVEL_KEYS = {"provider", "environment", "rail", "notary", "verify"}
RAIL_KEYS = {
    "inbox_dir",
    "message",
    "torii_base_url",
    "receipt_dir",
    "dry_run",
    "allow_default_profile",
    "allow_insecure_http",
    "bearer_token_file",
    "max_payload_bytes",
    "timeout_secs",
    "response_limit_bytes",
}
NOTARY_KEYS = {
    "export_dir",
    "endpoints",
    "receipt_dir",
    "all",
    "dry_run",
    "allow_insecure_http",
    "bearer_token_file",
    "timeout_secs",
    "response_limit_bytes",
}
VERIFY_KEYS = {
    "enabled",
    "receipts",
    "receipt_dirs",
    "include_stage_receipts",
    "allow_failed",
    "allow_insecure_http",
    "allow_default_profile",
    "require_source_files",
    "skip_on_stage_failure",
}


class CanaryError(RuntimeError):
    """Raised when a canary runbook is invalid."""


@dataclass(frozen=True)
class StagePlan:
    """One subprocess stage planned by the canary runner."""

    name: str
    argv: list[str]
    receipt_dir: Path | None = None
    dry_run: bool = False


@dataclass(frozen=True)
class StageResult:
    """Bounded subprocess result for one canary stage."""

    name: str
    started_at: str
    finished_at: str
    returncode: int
    command: list[str]
    stdout_preview: str
    stderr_preview: str
    stdout_truncated: bool
    stderr_truncated: bool
    receipt_dir: str | None
    timed_out: bool = False
    skipped: bool = False
    reason: str | None = None


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
        raise CanaryError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise CanaryError(f"{path} does not exist") from error
    mode = metadata.st_mode
    if stat.S_ISLNK(mode):
        raise CanaryError(f"{path} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise CanaryError(f"{path} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise CanaryError(f"{path} exceeds {max_bytes} byte JSON limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise CanaryError(f"{path} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise CanaryError(f"{path} exceeds {max_bytes} byte JSON limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise CanaryError(f"{path} exceeds {max_bytes} byte JSON limit")
        return raw
    except FileNotFoundError as error:
        raise CanaryError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise CanaryError(f"{path} must not be a symlink") from error
        raise CanaryError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise CanaryError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise CanaryError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise CanaryError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise CanaryError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise CanaryError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise CanaryError(f"{label} must use forward slashes")
    if ";" in raw:
        raise CanaryError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise CanaryError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise CanaryError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise CanaryError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise CanaryError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise CanaryError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise CanaryError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise CanaryError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise CanaryError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise CanaryError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise CanaryError(f"{label} must use forward slashes")
    if ";" in raw:
        raise CanaryError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise CanaryError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise CanaryError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise CanaryError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise CanaryError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise CanaryError(f"{label} must not contain dot or parent segments")


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise CanaryError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise CanaryError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise CanaryError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise CanaryError(f"{label} must not contain encoded semicolon parameters")
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise CanaryError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise CanaryError(f"{label} must not contain encoded percent characters")
        index += 3


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise CanaryError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in arg):
            raise CanaryError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise CanaryError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise CanaryError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise CanaryError("argument terminator is not supported")
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise CanaryError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise CanaryError(f"{arg} does not take a value")
        index += 1


def _preflight_output_cli_paths(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise CanaryError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise CanaryError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise CanaryError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise CanaryError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise CanaryError(f"{flag} must be a numeric value")
    if any(ord(ch) > 0x7E for ch in raw):
        raise CanaryError(f"{flag} must use printable ASCII")
    try:
        int(raw, 10) if integer else float(raw)
    except ValueError as error:
        raise CanaryError(f"{flag} must be a numeric value") from error


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
            raise CanaryError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise CanaryError(f"{flag} requires a numeric value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise CanaryError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise CanaryError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _ensure_text_output_target(path: Path) -> None:
    _reject_output_path_smuggling(path, "output path")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except FileExistsError as error:
        raise CanaryError(f"{path.parent} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise CanaryError(f"{path.parent} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise CanaryError(f"{path.parent} must be a directory")
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise CanaryError(f"{path} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise CanaryError(f"{path} must be a regular file")
        if metadata.st_nlink > 1:
            raise CanaryError(f"{path} must not be hard-linked")


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
            raise CanaryError(f"{current} must not be a symlink")


def _write_text_output(path: Path, text: str) -> None:
    _ensure_text_output_target(path)
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise CanaryError(f"{path.parent} must not be a symlink") from error
        raise CanaryError(f"{path.parent} must be a directory") from error

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
                raise CanaryError(f"{path} temp file must not be a symlink") from error
            raise CanaryError(
                f"cannot open temporary output for {path}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise CanaryError(f"{path} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise CanaryError(f"{path} temp file must not be hard-linked")
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


def _load_json(path: Path) -> Any:
    try:
        raw = _read_regular_file(path, max_bytes=MAX_CONFIG_JSON_BYTES)
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise CanaryError(f"{path} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise CanaryError(f"{path} is not valid JSON: {error}") from error
    _reject_json_surrogates(value)
    return value


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise CanaryError("JSON object contains duplicate key")
        seen.add(key)
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise CanaryError(f"JSON contains non-finite numeric constant {value}")


def _reject_json_surrogates(value: Any) -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise CanaryError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        for item in value:
            _reject_json_surrogates(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            _reject_json_surrogates(key)
            _reject_json_surrogates(item)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise CanaryError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        if any(
            _is_secret_looking_key(key)
            or _is_control_bearing_key(key)
            or any(ord(ch) > 0x7E for ch in str(key))
            for key in unknown
        ):
            raise CanaryError(f"{label} contains unknown keys")
        raise CanaryError(f"{label} contains unknown keys: {', '.join(unknown)}")


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


def _is_control_bearing_key(value: Any) -> bool:
    return any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in str(value))


def _is_secret_looking_identifier(value: Any) -> bool:
    return any(
        SECRET_IDENTIFIER_PATTERN.search(candidate)
        for candidate in _secret_scan_values(str(value))
    )


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_identifier(value):
        raise CanaryError(f"{label} must not contain secret-looking material")


def _reject_non_ascii_context(value: str, label: str) -> None:
    if any(ord(ch) > 0x7E for ch in value):
        raise CanaryError(f"{label} must use printable ASCII")


def _required_context_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(value, key, label)
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise CanaryError(f"{label}.{key} must be a non-empty string")
    _reject_control_chars(raw, f"{label}.{key}")
    if raw != raw.strip():
        raise CanaryError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _optional_string(value: dict[str, Any], key: str, label: str) -> str | None:
    if key not in value:
        return None
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise CanaryError(f"{label}.{key} must be a non-empty string when provided")
    _reject_control_chars(raw, f"{label}.{key}")
    if raw != raw.strip():
        raise CanaryError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _reject_control_chars(value: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise CanaryError(f"{label} must not contain control characters")


def _optional_bool(
    value: dict[str, Any], key: str, label: str, *, default: bool = False
) -> bool:
    raw = value.get(key, default)
    if not isinstance(raw, bool):
        raise CanaryError(f"{label}.{key} must be a boolean")
    return raw


def _policy_bool(
    value: dict[str, Any],
    key: str,
    label: str,
    *,
    default: bool = False,
    require_explicit_policy: bool,
) -> bool:
    if require_explicit_policy and key not in value:
        raise CanaryError(
            f"{label}.{key} must be explicitly set when --require-explicit-policy is used"
        )
    return _optional_bool(value, key, label, default=default)


def _optional_positive_int(
    value: dict[str, Any], key: str, label: str
) -> int | None:
    if key not in value:
        return None
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise CanaryError(f"{label}.{key} must be a positive integer")
    return raw


def _optional_positive_number(
    value: dict[str, Any], key: str, label: str
) -> float | None:
    if key not in value:
        return None
    raw = value.get(key)
    if (
        isinstance(raw, bool)
        or not isinstance(raw, (int, float))
        or not math.isfinite(float(raw))
        or raw <= 0
    ):
        raise CanaryError(f"{label}.{key} must be a positive number")
    return float(raw)


def _require_positive_finite_number(value: float, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise CanaryError(f"{label} must be a positive finite number")
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise CanaryError(f"{label} must be a positive finite number")
    return parsed


def _string_list(
    value: dict[str, Any],
    key: str,
    label: str,
    *,
    require_explicit_policy: bool = False,
) -> list[str]:
    if key not in value:
        if require_explicit_policy:
            raise CanaryError(
                f"{label}.{key} must be explicitly recorded as an array "
                "when --require-explicit-policy is used"
            )
        return []
    raw = value[key]
    if not isinstance(raw, list):
        raise CanaryError(f"{label}.{key} must be an array of strings")
    result: list[str] = []
    for offset, item in enumerate(raw):
        if not isinstance(item, str) or not item.strip():
            raise CanaryError(f"{label}.{key}[{offset}] must be a non-empty string")
        _reject_control_chars(item, f"{label}.{key}[{offset}]")
        if item != item.strip():
            raise CanaryError(
                f"{label}.{key}[{offset}] must not have surrounding whitespace"
            )
        result.append(item)
    _reject_duplicate_strings(result, f"{label}.{key}")
    return result


def _reject_duplicate_strings(values: list[str], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, value in enumerate(values):
        if value in seen:
            raise CanaryError(f"{label}[{offset}] duplicates {label}[{seen[value]}]")
        seen[value] = offset


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path.resolve())
        if key in seen:
            raise CanaryError(f"{label}[{offset}] duplicates {label}[{seen[key]}]")
        seen[key] = offset


def _reject_receipts_covered_by_dirs(
    receipts: list[Path],
    receipt_dirs: list[Path],
) -> None:
    dirs_by_key = {
        str(receipt_dir.resolve()): offset
        for offset, receipt_dir in enumerate(receipt_dirs)
    }
    for offset, receipt in enumerate(receipts):
        dir_offset = dirs_by_key.get(str(receipt.parent.resolve()))
        if dir_offset is not None:
            raise CanaryError(
                f"verify.receipts[{offset}] is already covered by "
                f"verify.receipt_dirs[{dir_offset}]"
            )


def _reject_receipts_from_stage_dirs(
    receipts: list[Path],
    stage_receipt_dirs: list[Path],
) -> None:
    stage_dirs = {str(receipt_dir.resolve()) for receipt_dir in stage_receipt_dirs}
    for offset, receipt in enumerate(receipts):
        if str(receipt.parent.resolve()) in stage_dirs:
            raise CanaryError(
                f"verify.receipts[{offset}] must not replace a generated "
                "stage receipt_dir"
            )


def _validate_path_string(
    raw: str,
    label: str,
    *,
    allow_runtime_secret_path: bool = False,
) -> None:
    _reject_control_chars(raw, label)
    if any(ord(ch) > 0x7E for ch in raw):
        raise CanaryError(f"{label} must use printable ASCII")
    if any(ch.isspace() for ch in raw):
        raise CanaryError(f"{label} must not contain whitespace")
    if "\\" in raw:
        raise CanaryError(f"{label} must use forward slashes")
    if ";" in raw:
        raise CanaryError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise CanaryError(f"{label} must not contain URI or drive prefixes")
    if not allow_runtime_secret_path and (
        _contains_secret_material(raw) or _contains_secret_identifier_material(raw)
    ):
        raise CanaryError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    for offset, part in enumerate(parts):
        if part == "" and offset != 0:
            raise CanaryError(f"{label} must not contain empty path segments")
        if part.startswith("-"):
            raise CanaryError(f"{label} must not contain leading-dash path segments")
        if part in {".", ".."}:
            raise CanaryError(f"{label} must not contain dot or parent segments")


def _path_from_config(
    config_dir: Path,
    raw: str,
    label: str,
    *,
    allow_runtime_secret_path: bool = False,
) -> Path:
    _validate_path_string(
        raw,
        label,
        allow_runtime_secret_path=allow_runtime_secret_path,
    )
    path = Path(raw).expanduser()
    if path.is_absolute():
        return path
    candidate = config_dir / path
    resolved_parent = candidate.parent.resolve()
    root = config_dir.resolve()
    if not resolved_parent.is_relative_to(root):
        raise CanaryError(f"{label} relative paths must stay under {config_dir.resolve()}")
    return resolved_parent / candidate.name


def _validate_endpoint_url(
    url: str,
    label: str,
    *,
    allow_insecure_http: bool,
    allow_template_canary: bool = False,
) -> None:
    if len(url) > MAX_HTTP_URL_CHARS:
        raise CanaryError(f"{label} must be no longer than {MAX_HTTP_URL_CHARS} characters")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
        raise CanaryError(f"{label} must not contain control characters")
    _reject_url_percent_encoding_smuggling(url, label)
    if any(ch.isspace() for ch in url):
        raise CanaryError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise CanaryError(f"{label} is not a valid URL") from error
    if parsed.scheme != "https" and not (parsed.scheme == "http" and allow_insecure_http):
        raise CanaryError(f"{label} must use HTTPS")
    try:
        port = parsed.port
    except ValueError as error:
        raise CanaryError(f"{label} has invalid port") from error
    port_text = _raw_url_port_text(parsed)
    if port_text == "":
        raise CanaryError(f"{label} must not include an empty port")
    if port_text is not None:
        if len(port_text) > 1 and port_text.startswith("0"):
            raise CanaryError(f"{label} port must not contain leading zeros")
        if port == 0:
            raise CanaryError(f"{label} port must be positive")
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise CanaryError(f"{label} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None:
        raise CanaryError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise CanaryError(f"{label} must not contain credentials")
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise CanaryError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise CanaryError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise CanaryError(f"{label} host must not end with a dot")
    if any(ord(ch) > 0x7E for ch in raw_host):
        raise CanaryError(f"{label} host must use printable ASCII")
    _reject_secret_looking_identifier(raw_host, f"{label} host")
    _validate_host_labels(raw_host, label)
    _reject_local_url_host(parsed, label, allow_insecure_http=allow_insecure_http)
    if parsed.params or parsed.query or parsed.fragment:
        raise CanaryError(f"{label} must not contain params, query, or fragment")
    _validate_url_path(parsed, label)
    _reject_reserved_placeholder_url_host(parsed, label)
    if not allow_template_canary:
        _reject_template_canary_url_host(parsed, label)


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
            raise CanaryError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise CanaryError(
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
        raise CanaryError(f"{label} host must be a valid IP address")
    if len(raw_host) > 253:
        raise CanaryError(f"{label} host must be at most 253 characters")
    _reject_legacy_ipv4_host_notation(raw_host, label)
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise CanaryError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise CanaryError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise CanaryError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise CanaryError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise CanaryError(
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
        raise CanaryError(f"{label} host must not use legacy IPv4 numeric notation")


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
        raise CanaryError(f"{label} must not use localhost")
    if _host_uses_rebinding_suffix(hostname):
        raise CanaryError(f"{label} must not use local/private rebinding hostnames")
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        return
    if not address.is_global:
        raise CanaryError(f"{label} must not use local, private, or reserved IP addresses")
    if _address_embeds_non_global_ipv4(address):
        raise CanaryError(f"{label} must not embed local, private, or reserved IPv4 addresses")


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
        raise CanaryError(f"{label} must not use reserved placeholder hostnames")


def _reject_template_canary_url_host(
    parsed: urllib.parse.ParseResult,
    label: str,
) -> None:
    hostname = (parsed.hostname or "").strip().lower()
    if _host_uses_template_canary_suffix(hostname):
        raise CanaryError(f"{label} must not use template canary hostnames")


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
        raise CanaryError(f"{label} path must use printable ASCII")
    if "\\" in path:
        raise CanaryError(f"{label} path must use forward slashes")
    if ";" in path:
        raise CanaryError(f"{label} path must not contain semicolon parameters")
    if any(token in path for token in (":", "@", "[", "]")):
        raise CanaryError(f"{label} path must not contain URL delimiter characters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise CanaryError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise CanaryError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
        raise CanaryError(f"{label} path must not contain secret-looking material")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise CanaryError(f"{label} path must not contain encoded dot or separator characters")
    if "%3b" in lowered:
        raise CanaryError(f"{label} path must not contain encoded semicolon parameters")
    if any(token in lowered for token in ("%23", "%3a", "%3f", "%40", "%5b", "%5d")):
        raise CanaryError(f"{label} path must not contain encoded URL delimiter characters")
    if "%25" in lowered:
        raise CanaryError(f"{label} path must not contain encoded percent characters")
    if re.search(r"%[89a-f][0-9a-f]", lowered):
        raise CanaryError(f"{label} path must not contain percent-encoded non-ASCII bytes")


def _script(name: str) -> str:
    return str(SCRIPT_DIR / name)


def _append_path(argv: list[str], flag: str, path: Path | None) -> None:
    if path is not None:
        argv.extend([flag, str(path)])


def _append_bool(argv: list[str], flag: str, enabled: bool) -> None:
    if enabled:
        argv.append(flag)


def _append_value(argv: list[str], flag: str, value: object | None) -> None:
    if value is not None:
        argv.extend([flag, str(value)])


def _build_rail_stage(
    config_dir: Path,
    raw: Any,
    *,
    require_explicit_policy: bool,
    allow_template_canary_endpoints: bool,
) -> StagePlan:
    rail = _require_object(raw, "rail")
    _reject_unknown_keys(rail, RAIL_KEYS, "rail")

    inbox_dir = _path_from_config(
        config_dir,
        _required_string(rail, "inbox_dir", "rail"),
        "rail.inbox_dir",
    )
    message = _optional_string(rail, "message", "rail")
    message_path = (
        _path_from_config(config_dir, message, "rail.message")
        if message is not None
        else None
    )
    torii_base_url = _required_string(rail, "torii_base_url", "rail")
    allow_insecure_http = _policy_bool(
        rail,
        "allow_insecure_http",
        "rail",
        require_explicit_policy=require_explicit_policy,
    )
    _validate_endpoint_url(
        torii_base_url,
        "rail.torii_base_url",
        allow_insecure_http=allow_insecure_http,
        allow_template_canary=allow_template_canary_endpoints,
    )
    receipt_dir_raw = _optional_string(rail, "receipt_dir", "rail")
    receipt_dir = (
        _path_from_config(config_dir, receipt_dir_raw, "rail.receipt_dir")
        if receipt_dir_raw is not None
        else inbox_dir / "receipts"
    )
    bearer_raw = _optional_string(rail, "bearer_token_file", "rail")
    bearer_token_file = (
        _path_from_config(
            config_dir,
            bearer_raw,
            "rail.bearer_token_file",
            allow_runtime_secret_path=True,
        )
        if bearer_raw is not None
        else None
    )
    dry_run = _policy_bool(
        rail,
        "dry_run",
        "rail",
        require_explicit_policy=require_explicit_policy,
    )
    allow_default_profile = _policy_bool(
        rail,
        "allow_default_profile",
        "rail",
        require_explicit_policy=require_explicit_policy,
    )

    argv = [
        sys.executable,
        _script("iso_rail_gateway_adapter.py"),
        "--inbox-dir",
        str(inbox_dir),
        "--torii-base-url",
        torii_base_url,
        "--receipt-dir",
        str(receipt_dir),
    ]
    _append_path(argv, "--message", message_path)
    _append_bool(argv, "--dry-run", dry_run)
    _append_bool(argv, "--allow-default-profile", allow_default_profile)
    _append_bool(argv, "--allow-insecure-http", allow_insecure_http)
    _append_path(argv, "--bearer-token-file", bearer_token_file)
    _append_value(argv, "--max-payload-bytes", _optional_positive_int(rail, "max_payload_bytes", "rail"))
    _append_value(argv, "--timeout-secs", _optional_positive_number(rail, "timeout_secs", "rail"))
    _append_value(
        argv,
        "--response-limit-bytes",
        _optional_positive_int(rail, "response_limit_bytes", "rail"),
    )
    return StagePlan("rail", argv, receipt_dir=receipt_dir, dry_run=dry_run)


def _build_notary_stage(
    config_dir: Path,
    raw: Any,
    *,
    require_explicit_policy: bool,
    allow_template_canary_endpoints: bool,
) -> StagePlan:
    notary = _require_object(raw, "notary")
    _reject_unknown_keys(notary, NOTARY_KEYS, "notary")

    export_dir = _path_from_config(
        config_dir,
        _required_string(notary, "export_dir", "notary"),
        "notary.export_dir",
    )
    endpoints = _string_list(
        notary,
        "endpoints",
        "notary",
        require_explicit_policy=require_explicit_policy,
    )
    dry_run = _policy_bool(
        notary,
        "dry_run",
        "notary",
        require_explicit_policy=require_explicit_policy,
    )
    if not dry_run and not endpoints:
        raise CanaryError("notary.endpoints must contain at least one endpoint unless dry_run is true")
    allow_insecure_http = _policy_bool(
        notary,
        "allow_insecure_http",
        "notary",
        require_explicit_policy=require_explicit_policy,
    )
    for offset, endpoint in enumerate(endpoints):
        _validate_endpoint_url(
            endpoint,
            f"notary.endpoints[{offset}]",
            allow_insecure_http=allow_insecure_http,
            allow_template_canary=allow_template_canary_endpoints,
        )
    receipt_dir_raw = _optional_string(notary, "receipt_dir", "notary")
    receipt_dir = (
        _path_from_config(config_dir, receipt_dir_raw, "notary.receipt_dir")
        if receipt_dir_raw is not None
        else export_dir / "receipts"
    )
    bearer_raw = _optional_string(notary, "bearer_token_file", "notary")
    bearer_token_file = (
        _path_from_config(
            config_dir,
            bearer_raw,
            "notary.bearer_token_file",
            allow_runtime_secret_path=True,
        )
        if bearer_raw is not None
        else None
    )

    argv = [
        sys.executable,
        _script("iso_audit_notary_adapter.py"),
        "--export-dir",
        str(export_dir),
        "--receipt-dir",
        str(receipt_dir),
    ]
    for endpoint in endpoints:
        argv.extend(["--endpoint", endpoint])
    _append_bool(
        argv,
        "--all",
        _policy_bool(
            notary,
            "all",
            "notary",
            require_explicit_policy=require_explicit_policy,
        ),
    )
    _append_bool(argv, "--dry-run", dry_run)
    _append_bool(
        argv,
        "--allow-insecure-http",
        allow_insecure_http,
    )
    _append_path(argv, "--bearer-token-file", bearer_token_file)
    _append_value(argv, "--timeout-secs", _optional_positive_number(notary, "timeout_secs", "notary"))
    _append_value(
        argv,
        "--response-limit-bytes",
        _optional_positive_int(notary, "response_limit_bytes", "notary"),
    )
    return StagePlan("notary", argv, receipt_dir=receipt_dir, dry_run=dry_run)


def _build_verify_stage(
    config_dir: Path,
    raw: Any,
    stage_receipt_dirs: list[Path],
    *,
    prior_failure: bool,
    require_explicit_policy: bool,
) -> StagePlan | None:
    if require_explicit_policy and raw is None:
        raise CanaryError("verify must be configured when --require-explicit-policy is used")
    verify = {} if raw is None else _require_object(raw, "verify")
    _reject_unknown_keys(verify, VERIFY_KEYS, "verify")
    if not _policy_bool(
        verify,
        "enabled",
        "verify",
        default=True,
        require_explicit_policy=require_explicit_policy,
    ):
        return None
    skip_on_failure = _policy_bool(
        verify,
        "skip_on_stage_failure",
        "verify",
        default=True,
        require_explicit_policy=require_explicit_policy,
    )
    if prior_failure and skip_on_failure:
        return StagePlan(
            "verify",
            [],
            receipt_dir=None,
            dry_run=False,
        )

    include_stage_receipts = _policy_bool(
        verify,
        "include_stage_receipts",
        "verify",
        default=True,
        require_explicit_policy=require_explicit_policy,
    )
    receipt_dirs = [
        _path_from_config(config_dir, item, f"verify.receipt_dirs[{offset}]")
        for offset, item in enumerate(
            _string_list(
                verify,
                "receipt_dirs",
                "verify",
                require_explicit_policy=require_explicit_policy,
            )
        )
    ]
    if include_stage_receipts:
        receipt_dirs.extend(stage_receipt_dirs)
    receipts = [
        _path_from_config(config_dir, item, f"verify.receipts[{offset}]")
        for offset, item in enumerate(
            _string_list(
                verify,
                "receipts",
                "verify",
                require_explicit_policy=require_explicit_policy,
            )
        )
    ]
    _reject_duplicate_paths(receipt_dirs, "verify.receipt_dirs")
    _reject_duplicate_paths(receipts, "verify.receipts")
    _reject_receipts_covered_by_dirs(receipts, receipt_dirs)
    _reject_receipts_from_stage_dirs(receipts, stage_receipt_dirs)
    if not receipt_dirs and not receipts:
        raise CanaryError("verify requires generated stage receipts or explicit receipts/receipt_dirs")

    argv = [
        sys.executable,
        _script("iso_operator_receipt_verify.py"),
    ]
    for receipt in receipts:
        argv.extend(["--receipt", str(receipt)])
    for receipt_dir in receipt_dirs:
        argv.extend(["--receipt-dir", str(receipt_dir)])
    _append_bool(
        argv,
        "--allow-failed",
        _policy_bool(
            verify,
            "allow_failed",
            "verify",
            require_explicit_policy=require_explicit_policy,
        ),
    )
    _append_bool(
        argv,
        "--allow-insecure-http",
        _policy_bool(
            verify,
            "allow_insecure_http",
            "verify",
            require_explicit_policy=require_explicit_policy,
        ),
    )
    _append_bool(
        argv,
        "--allow-default-profile",
        _policy_bool(
            verify,
            "allow_default_profile",
            "verify",
            require_explicit_policy=require_explicit_policy,
        ),
    )
    _append_bool(
        argv,
        "--require-source-files",
        _policy_bool(
            verify,
            "require_source_files",
            "verify",
            default=True,
            require_explicit_policy=require_explicit_policy,
        ),
    )
    return StagePlan("verify", argv)


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
        raise CanaryError("output limit bytes must be positive")
    timeout_secs = _require_positive_finite_number(timeout_secs, "stage timeout seconds")
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


def _redacted_command(argv: list[str]) -> list[str]:
    redacted: list[str] = []
    redact_next = False
    for item in argv:
        if redact_next:
            redacted.append("<runtime-token-file>")
            redact_next = False
            continue
        prefix = "--bearer-token-file="
        if item.startswith(prefix):
            redacted.append(prefix + "<runtime-token-file>")
            continue
        redacted.append(item)
        if item == "--bearer-token-file":
            redact_next = True
    return redacted


def _run_stage(stage: StagePlan, output_limit_bytes: int, stage_timeout_secs: float) -> StageResult:
    started_at = dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()
    (
        returncode,
        stdout,
        stdout_truncated,
        stderr,
        stderr_truncated,
        timed_out,
    ) = _run_command_bounded(
        stage.argv,
        output_limit_bytes,
        stage_timeout_secs,
    )
    finished_at = dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()
    return StageResult(
        name=stage.name,
        started_at=started_at,
        finished_at=finished_at,
        returncode=returncode,
        command=_redacted_command(stage.argv),
        stdout_preview=stdout,
        stderr_preview=stderr,
        stdout_truncated=stdout_truncated,
        stderr_truncated=stderr_truncated,
        receipt_dir=str(stage.receipt_dir) if stage.receipt_dir is not None else None,
        timed_out=timed_out,
    )


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


def _contains_secret_output_material(value: str) -> bool:
    return _contains_secret_material(value) or _contains_secret_identifier_material(value)


def _contains_unsafe_control_chars(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 or ord(ch) == 0x7F) and ch not in SAFE_OUTPUT_CONTROL_CHARS
        for ch in value
    )


def _reject_unsafe_stage_output(results: list[StageResult]) -> None:
    for result in results:
        if _contains_unsafe_control_chars(result.stdout_preview):
            raise CanaryError(
                f"stage {result.name} stdout_preview contains unsafe control characters"
            )
        if _contains_secret_output_material(result.stdout_preview):
            raise CanaryError(
                f"stage {result.name} stdout_preview contains secret-looking material"
            )
        if _contains_unsafe_control_chars(result.stderr_preview):
            raise CanaryError(
                f"stage {result.name} stderr_preview contains unsafe control characters"
            )
        if _contains_secret_output_material(result.stderr_preview):
            raise CanaryError(
                f"stage {result.name} stderr_preview contains secret-looking material"
            )


def _stage_failed(result: StageResult) -> bool:
    return (
        result.returncode != 0
        or result.stdout_truncated
        or result.stderr_truncated
        or (result.returncode == 0 and bool(result.stderr_preview.strip()))
    )


def _skipped_verify_result(reason: str) -> StageResult:
    timestamp = dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()
    return StageResult(
        name="verify",
        started_at=timestamp,
        finished_at=timestamp,
        returncode=0,
        command=[],
        stdout_preview="",
        stderr_preview="",
        stdout_truncated=False,
        stderr_truncated=False,
        receipt_dir=None,
        timed_out=False,
        skipped=True,
        reason=reason,
    )


def _result_to_json(result: StageResult) -> dict[str, Any]:
    return {
        "name": result.name,
        "started_at": result.started_at,
        "finished_at": result.finished_at,
        "returncode": result.returncode,
        "command": result.command,
        "stdout_preview": result.stdout_preview,
        "stderr_preview": result.stderr_preview,
        "stdout_truncated": result.stdout_truncated,
        "stderr_truncated": result.stderr_truncated,
        "receipt_dir": result.receipt_dir,
        "timed_out": result.timed_out,
        "skipped": result.skipped,
        "reason": result.reason,
    }


def _plan_to_json(stage: StagePlan) -> dict[str, Any]:
    return {
        "name": stage.name,
        "command": _redacted_command(stage.argv),
        "receipt_dir": str(stage.receipt_dir) if stage.receipt_dir is not None else None,
        "dry_run": stage.dry_run,
    }


def build_stage_plans(
    config_path: Path,
    config: dict[str, Any],
    *,
    require_explicit_policy: bool,
    allow_template_canary_endpoints: bool,
) -> tuple[str, str, list[StagePlan], Any]:
    """Validate a runbook and return provider metadata plus non-verify stages."""

    _reject_unknown_keys(config, TOP_LEVEL_KEYS, "config")
    provider = _required_context_string(config, "provider", "config")
    environment = _required_context_string(config, "environment", "config")
    config_dir = config_path.resolve().parent

    stages: list[StagePlan] = []
    if "rail" in config:
        stages.append(
            _build_rail_stage(
                config_dir,
                config["rail"],
                require_explicit_policy=require_explicit_policy,
                allow_template_canary_endpoints=allow_template_canary_endpoints,
            )
        )
    if "notary" in config:
        stages.append(
            _build_notary_stage(
                config_dir,
                config["notary"],
                require_explicit_policy=require_explicit_policy,
                allow_template_canary_endpoints=allow_template_canary_endpoints,
            )
        )
    if not stages:
        raise CanaryError("configure at least one of rail or notary")
    _reject_duplicate_paths(
        [stage.receipt_dir for stage in stages if stage.receipt_dir is not None],
        "stage.receipt_dir",
    )
    return provider, environment, stages, config.get("verify")


def run(args: argparse.Namespace) -> int:
    config_path = args.config
    config = _require_object(_load_json(config_path), "config")
    resolved_config_path = config_path.resolve()
    provider, environment, stages, verify_config = build_stage_plans(
        resolved_config_path,
        config,
        require_explicit_policy=args.require_explicit_policy,
        allow_template_canary_endpoints=args.plan_only,
    )
    if (
        isinstance(args.output_limit_bytes, bool)
        or not isinstance(args.output_limit_bytes, int)
        or args.output_limit_bytes <= 0
    ):
        raise CanaryError("--output-limit-bytes must be positive")
    stage_timeout_secs = _require_positive_finite_number(
        args.stage_timeout_secs, "--stage-timeout-secs"
    )
    if args.summary_out is not None:
        _ensure_text_output_target(args.summary_out)

    started_at = dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat()
    if args.plan_only:
        stage_receipt_dirs = [
            stage.receipt_dir
            for stage in stages
            if stage.receipt_dir is not None and not stage.dry_run
        ]
        verify_stage = _build_verify_stage(
            resolved_config_path.parent,
            verify_config,
            stage_receipt_dirs,
            prior_failure=False,
            require_explicit_policy=args.require_explicit_policy,
        )
        planned_stages = [_plan_to_json(stage) for stage in stages]
        if verify_stage is not None:
            planned_stages.append(_plan_to_json(verify_stage))
        summary: dict[str, Any] = {
            "version": CANARY_SUMMARY_VERSION,
            "provider": provider,
            "environment": environment,
            "config_path": str(resolved_config_path),
            "started_at": started_at,
            "finished_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
            "ok": True,
            "plan_only": True,
            "policy": {
                "require_explicit_policy": args.require_explicit_policy,
            },
            "planned_stages": planned_stages,
        }
        summary["summary_sha256"] = sha256_hex(_canonical_json_bytes(summary))
        text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
        if args.summary_out is not None:
            _write_text_output(args.summary_out, text)
        print(text, end="")
        return 0

    results: list[StageResult] = []
    stage_receipt_dirs: list[Path] = []
    prior_failure = False
    for stage in stages:
        result = _run_stage(stage, args.output_limit_bytes, stage_timeout_secs)
        results.append(result)
        if stage.receipt_dir is not None and not stage.dry_run:
            stage_receipt_dirs.append(stage.receipt_dir)
        if _stage_failed(result):
            prior_failure = True

    verify_stage = _build_verify_stage(
        resolved_config_path.parent,
        verify_config,
        stage_receipt_dirs,
        prior_failure=prior_failure,
        require_explicit_policy=args.require_explicit_policy,
    )
    if verify_stage is not None:
        if not verify_stage.argv:
            results.append(_skipped_verify_result("skipped because an earlier stage failed"))
        else:
            verify_result = _run_stage(
                verify_stage,
                args.output_limit_bytes,
                stage_timeout_secs,
            )
            results.append(verify_result)
            if _stage_failed(verify_result):
                prior_failure = True

    _reject_unsafe_stage_output(results)

    summary: dict[str, Any] = {
        "version": CANARY_SUMMARY_VERSION,
        "provider": provider,
        "environment": environment,
        "config_path": str(resolved_config_path),
        "started_at": started_at,
        "finished_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": not prior_failure,
        "plan_only": False,
        "policy": {
            "require_explicit_policy": args.require_explicit_policy,
        },
        "stages": [_result_to_json(result) for result in results],
    }
    summary["summary_sha256"] = sha256_hex(_canonical_json_bytes(summary))
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        _write_text_output(args.summary_out, text)
    print(text, end="")
    return 0 if summary["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Run ISO 20022 rail/notary adapters and verify canary receipts.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--config",
        required=True,
        type=Path,
        help="JSON runbook with provider, environment, and rail/notary/verify sections.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the canary summary JSON.",
    )
    parser.add_argument(
        "--plan-only",
        action="store_true",
        help="Validate the runbook and print redacted planned child commands without executing them.",
    )
    parser.add_argument(
        "--output-limit-bytes",
        type=int,
        default=DEFAULT_OUTPUT_LIMIT_BYTES,
        help="Maximum stdout/stderr bytes retained per child stage in the summary.",
    )
    parser.add_argument(
        "--stage-timeout-secs",
        type=float,
        default=DEFAULT_STAGE_TIMEOUT_SECS,
        help="Maximum wall-clock seconds allowed for each child stage.",
    )
    parser.add_argument(
        "--require-explicit-policy",
        action="store_true",
        help="Require all runbook policy booleans to be explicit and record that in the summary.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        _preflight_raw_cli_secrets(
            argv,
            {
                "--config",
                "--output-limit-bytes",
                "--stage-timeout-secs",
                "--summary-out",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--plan-only",
                "--require-explicit-policy",
            },
        )
        _preflight_numeric_cli_values(
            argv,
            integer_flags={"--output-limit-bytes"},
            number_flags={"--stage-timeout-secs"},
        )
        _preflight_output_cli_paths(argv, {"--config", "--summary-out"})
        args = parser.parse_args(argv)
        return run(args)
    except CanaryError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
