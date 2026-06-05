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
import os
import stat
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Any


DEFAULT_MAX_PAYLOAD_BYTES = 4 * 1024 * 1024
DEFAULT_RESPONSE_LIMIT_BYTES = 64 * 1024
MAX_BEARER_TOKEN_BYTES = 8192
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
LEGACY_MESSAGE_TYPES = {"colr.007"}

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


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise AdapterError(f"JSON object contains duplicate key {key!r}")
        seen.add(key)
        result[key] = value
    return result


def _read_regular_file(path: Path) -> bytes:
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{path} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise AdapterError(f"{path} must be a regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise AdapterError(f"{path} must be a regular file")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            return handle.read()
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise AdapterError(f"{path} must not be a symlink") from error
        raise AdapterError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _ensure_input_directory(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{label} {path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{label} {path} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise AdapterError(f"{label} {path} must be a directory")


def _ensure_output_directory(path: Path, label: str) -> None:
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
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise AdapterError(f"{path} must not be a symlink")
        if not stat.S_ISREG(mode):
            raise AdapterError(f"{path} must be a regular file")


def _write_text_output(path: Path, text: str) -> None:
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
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_CLOEXEC", 0)
        try:
            fd = os.open(path.name, flags | nofollow, 0o666, dir_fd=parent_fd)
        except OSError as error:
            if error.errno == errno.ELOOP:
                raise AdapterError(f"{path} must not be a symlink") from error
            raise AdapterError(f"cannot open {path} for writing: {error.strerror}") from error
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise AdapterError(f"{path} must be a regular file")
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
    finally:
        if fd >= 0:
            os.close(fd)
        os.close(parent_fd)


def _absolute_path_without_resolving_leaf(path: Path) -> Path:
    return path if path.is_absolute() else Path.cwd() / path


def _load_json(path: Path) -> Any:
    raw = _read_regular_file(path)
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error


def _bounded_read(path: Path, max_bytes: int) -> bytes:
    if max_bytes <= 0:
        raise AdapterError("max payload bytes must be positive")
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise AdapterError(f"{path} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise AdapterError(f"{path} must be a regular file")
    size = metadata.st_size
    if size <= 0:
        raise AdapterError(f"{path} is empty")
    if size > max_bytes:
        raise AdapterError(f"{path} exceeds {max_bytes} byte payload limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise AdapterError(f"{path} must be a regular file")
        size = opened.st_size
        if size <= 0:
            raise AdapterError(f"{path} is empty")
        if size > max_bytes:
            raise AdapterError(f"{path} exceeds {max_bytes} byte payload limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            return handle.read()
    except FileNotFoundError as error:
        raise AdapterError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise AdapterError(f"{path} must not be a symlink") from error
        raise AdapterError(f"cannot open {path} for reading: {error.strerror}") from error
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

    _validate_path_argument(str(xml_path.name), f"{xml_path} filename")
    if xml_path.suffix.lower() != ".xml":
        raise AdapterError(f"{xml_path} must use a .xml suffix")
    sidecar_path = xml_path.with_suffix(xml_path.suffix + ".json")
    sidecar = _load_json(sidecar_path)
    if not isinstance(sidecar, dict):
        raise AdapterError(f"{sidecar_path} must contain a JSON object")

    payload = _bounded_read(xml_path, max_payload_bytes)
    actual_sha256 = sha256_hex(payload)
    expected_sha256 = sidecar.get("payload_sha256")
    if expected_sha256 != actual_sha256:
        raise AdapterError(
            f"{xml_path} payload_sha256 mismatch: expected {expected_sha256}, got {actual_sha256}"
        )
    if not _is_lower_hex_sha256(expected_sha256):
        raise AdapterError(f"{sidecar_path} payload_sha256 must be lowercase SHA-256 hex")

    message_type = sidecar.get("message_type")
    if not isinstance(message_type, str) or message_type not in ENDPOINTS:
        raise AdapterError(f"{sidecar_path} has unsupported message_type {message_type!r}")
    if message_type in LEGACY_MESSAGE_TYPES and not allow_legacy_colr007:
        raise AdapterError(
            f"{sidecar_path} uses legacy message_type {message_type!r}; "
            "use colr.012 for production collateral substitution confirmations"
        )

    profile_present = "profile" in sidecar
    if not profile_present:
        if not allow_default_profile:
            raise AdapterError(f"{sidecar_path} must specify profile for live rail submission")
        profile = None
    else:
        profile = sidecar.get("profile")
    if profile_present and (not isinstance(profile, str) or not profile.strip()):
        raise AdapterError(f"{sidecar_path} profile must be a non-empty string")
    if isinstance(profile, str):
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in profile):
            raise AdapterError(f"{sidecar_path} profile must not contain control characters")
        if profile != profile.strip():
            raise AdapterError(
                f"{sidecar_path} profile must not have surrounding whitespace"
            )
        if any(ch.isspace() for ch in profile):
            raise AdapterError(f"{sidecar_path} profile must not contain whitespace")

    rail_message_id_present = "rail_message_id" in sidecar
    rail_message_id = None
    if rail_message_id_present:
        rail_message_id = sidecar.get("rail_message_id")
    if rail_message_id_present and (
        not isinstance(rail_message_id, str) or not rail_message_id.strip()
    ):
        raise AdapterError(f"{sidecar_path} rail_message_id must be a non-empty string")
    if isinstance(rail_message_id, str):
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in rail_message_id):
            raise AdapterError(
                f"{sidecar_path} rail_message_id must not contain control characters"
            )
        if rail_message_id != rail_message_id.strip():
            raise AdapterError(
                f"{sidecar_path} rail_message_id must not have surrounding whitespace"
            )
        if any(ch.isspace() for ch in rail_message_id):
            raise AdapterError(
                f"{sidecar_path} rail_message_id must not contain whitespace"
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


def discover_messages(inbox_dir: Path) -> list[Path]:
    """Return inbound XML paths in deterministic order."""

    if not inbox_dir.is_dir():
        raise AdapterError(f"{inbox_dir} is not a directory")
    messages = sorted(path for path in inbox_dir.iterdir() if path.suffix.lower() == ".xml")
    if not messages:
        raise AdapterError(f"{inbox_dir} has no *.xml gateway messages")
    for path in messages:
        _validate_path_argument(str(path.name), f"{path} filename")
    return messages


def _validate_path_argument(raw: str, label: str) -> None:
    if any(ch.isspace() for ch in raw):
        raise AdapterError(f"{label} must not contain whitespace")
    if "\\" in raw:
        raise AdapterError(f"{label} must use forward slashes")
    if ";" in raw:
        raise AdapterError(f"{label} must not contain semicolon path parameters")
    parts = raw.split("/")
    for offset, part in enumerate(parts):
        if part == "" and offset != 0:
            raise AdapterError(f"{label} must not contain empty path segments")
        if part in {".", ".."}:
            raise AdapterError(f"{label} must not contain dot or parent segments")


def resolve_message_paths(inbox_dir: Path, message: str | None) -> list[Path]:
    """Resolve one explicit message or discover all messages under the inbox."""

    _ensure_input_directory(inbox_dir, "inbox_dir")
    inbox_root = inbox_dir.resolve()
    if message is None:
        return discover_messages(inbox_dir)
    _validate_path_argument(message, "--message path")
    raw_message = Path(message).expanduser()
    message_path = raw_message if raw_message.is_absolute() else inbox_dir / raw_message
    resolved_parent = message_path.parent.resolve()
    if not resolved_parent.is_relative_to(inbox_root):
        raise AdapterError(f"--message path {message} must stay under --inbox-dir {inbox_root}")
    return [resolved_parent / message_path.name]


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
        raise AdapterError(f"{label} has invalid port: {error}") from error
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


def _validate_url_host(parsed: urllib.parse.ParseResult, label: str) -> None:
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise AdapterError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise AdapterError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise AdapterError(f"{label} host must not end with a dot")
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise AdapterError(f"{label} host must be a valid IP address")
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
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise AdapterError(f"{label} path must not contain encoded dot or separator characters")
    if "%25" in lowered:
        raise AdapterError(f"{label} path must not contain encoded percent characters")


def _validate_base_url(base_url: str, allow_insecure_http: bool) -> str:
    _reject_url_control_chars(base_url, "Torii URL")
    _reject_url_percent_encoding_smuggling(base_url, "Torii URL")
    if base_url != base_url.strip():
        raise AdapterError("Torii URL must not have surrounding whitespace")
    if any(ch.isspace() for ch in base_url):
        raise AdapterError("Torii URL must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(base_url)
        hostname = parsed.hostname
    except ValueError as error:
        raise AdapterError(f"Torii URL {base_url} is not a valid URL: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise AdapterError(
                f"refusing insecure HTTP Torii URL {base_url}; pass --allow-insecure-http for local tests"
            )
        raise AdapterError(f"Torii URL {base_url} must use http or https")
    _validate_url_port(parsed, f"Torii URL {base_url}")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise AdapterError(f"Torii URL {base_url} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise AdapterError(f"Torii URL {base_url} must not contain credentials")
    _validate_url_host(parsed, f"Torii URL {base_url}")
    if parsed.params or parsed.query or parsed.fragment:
        raise AdapterError(
            f"Torii URL {base_url} must not contain params, query, or fragment"
        )
    _validate_url_path(parsed, f"Torii URL {base_url}")
    return base_url.rstrip("/")


def _load_bearer_token(path: Path | None) -> str | None:
    if path is None:
        return None
    raw = _read_regular_file(path)
    if len(raw) > MAX_BEARER_TOKEN_BYTES:
        raise AdapterError(f"bearer token file {path} exceeds {MAX_BEARER_TOKEN_BYTES} bytes")
    try:
        token = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise AdapterError(f"bearer token file {path} is not UTF-8") from error
    if not token:
        raise AdapterError(f"bearer token file {path} is empty")
    if token != token.strip():
        raise AdapterError(f"bearer token file {path} must not have surrounding whitespace")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in token):
        raise AdapterError(f"bearer token file {path} must not contain control characters")
    if any(ch.isspace() for ch in token):
        raise AdapterError(f"bearer token file {path} must not contain whitespace")
    return token


def torii_url(base_url: str, message: GatewayMessage) -> str:
    """Build the Torii ISO endpoint URL for a verified message."""

    return f"{base_url}/v1/iso20022/{ENDPOINTS[message.message_type]}"


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
        with urllib.request.urlopen(request, timeout=timeout_secs) as response:
            body = response.read(response_limit_bytes + 1)
            if len(body) > response_limit_bytes:
                raise AdapterError(
                    f"Torii response exceeded {response_limit_bytes} byte limit"
                )
            status_code = int(response.status)
    except urllib.error.HTTPError as error:
        try:
            body = error.read(response_limit_bytes + 1)
        finally:
            error.close()
        if len(body) > response_limit_bytes:
            raise AdapterError(f"Torii error response exceeded {response_limit_bytes} byte limit")
        return SubmitResult(
            status_code=int(error.code),
            ok=False,
            response_body_sha256=sha256_hex(body),
            response_body_preview=_response_preview(body),
            error=f"HTTP {error.code}",
        )
    except urllib.error.URLError as error:
        return SubmitResult(
            status_code=None,
            ok=False,
            response_body_sha256=None,
            response_body_preview=None,
            error=str(error.reason),
        )

    ok = 200 <= status_code <= 299
    return SubmitResult(
        status_code=status_code,
        ok=ok,
        response_body_sha256=sha256_hex(body),
        response_body_preview=_response_preview(body),
        error=None if ok else f"HTTP {status_code}",
    )


def _response_preview(body: bytes) -> str:
    return body[:4096].decode("utf-8", errors="replace")


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
    _write_text_output(path, json.dumps(receipt, indent=2) + "\n")
    return path


def receipt_output_path(receipt_dir: Path, message: GatewayMessage) -> Path:
    """Return the receipt path for one gateway message."""

    return receipt_dir / f"{message.payload_sha256}.receipt.json"


def run(args: argparse.Namespace) -> int:
    base_url = _validate_base_url(args.torii_base_url, args.allow_insecure_http)
    _ensure_input_directory(args.inbox_dir, "inbox_dir")
    inbox_dir = args.inbox_dir
    receipt_dir = _absolute_path_without_resolving_leaf(
        args.receipt_dir or inbox_dir / "receipts"
    )
    bearer_token = _load_bearer_token(args.bearer_token_file)
    paths = resolve_message_paths(inbox_dir, args.message)
    messages = [
        verify_message_file(
            path,
            max_payload_bytes=args.max_payload_bytes,
            allow_default_profile=args.allow_default_profile,
            allow_legacy_colr007=args.allow_legacy_colr007,
        )
        for path in paths
    ]

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
    for message in messages:
        _ensure_output_file_target(receipt_output_path(receipt_dir, message))

    failures = 0
    receipts: list[str] = []
    for message in messages:
        endpoint_url = torii_url(base_url, message)
        result = submit_message(
            base_url,
            message,
            timeout_secs=args.timeout_secs,
            response_limit_bytes=args.response_limit_bytes,
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
        description="Verify ISO 20022 rail-gateway file drops and submit them to Torii."
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
    args = parser.parse_args(argv)
    try:
        return run(args)
    except AdapterError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
