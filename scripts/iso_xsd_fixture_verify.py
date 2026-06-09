#!/usr/bin/env python3
"""Verify checked-in ISO 20022 XSD and XML fixture wiring.

Purpose:
  This offline preflight checks that the repository's ISO 20022 fixture
  manifest accurately binds checked-in Standards Editor XSDs to XML fixtures.
  It verifies schema target namespaces, the `Document` root, payload-root
  declarations, XML fixture namespaces, reviewed schema-only entries, and
  reviewed fixture entries whose official XSD package is still pending.

Prerequisites:
  Python 3.11+. No third party Python packages are required. This is a
  structural manifest and namespace preflight; it is not a full XSD validator.

Safety:
  The script is read-only unless ``--summary-out`` is supplied. It does not
  fetch schemas or XML documents over the network.
"""

from __future__ import annotations

import argparse
import base64
import binascii
import datetime as dt
import errno
import hashlib
import json
import math
import os
import re
import secrets
import shutil
import stat
import subprocess
import sys
import threading
import urllib.parse
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any


MANIFEST_VERSION = 1
SUMMARY_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
XML_SCHEMA_NS = "http://www.w3.org/2001/XMLSchema"
ISO_NAMESPACE_PREFIX = "urn:iso:std:iso:20022:tech:xsd:"
UNSUPPORTED_SCHEMA_COMPOSITION_CHILDREN = {
    "import",
    "include",
    "redefine",
    "override",
}
MESSAGE_DEF_ID_RE = re.compile(r"^[a-z]{4}\.[0-9]{3}\.[0-9]{3}\.[0-9]{2}$")
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.[0-9]{3}$")
PROFILE_CURRENCY_RE = re.compile(r"^[A-Z]{3}$")
SOURCE_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SOURCE_REPOSITORY_RE = re.compile(
    r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"
)
PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS = {
    "dummy",
    "example",
    "example-org",
    "example-owner",
    "example-repo",
    "fake",
    "placeholder",
    "replace-before-production",
    "sample",
    "template",
    "test",
    "operator-canary",
    "operator-canary-bank",
}
PROFILE_DIRECTIONS = {"inbound", "outbound", "follow-up"}
PROFILE_RAILS = {
    "generic-iso20022",
    "swift-cbpr-plus",
    "fedwire-funds",
    "sepa-sct-inst",
    "securities-csd",
}
PROFILE_SIGNATURE_POLICIES = {
    "record-only",
    "reject-unsupported",
    "require-verified",
}
PROFILE_REFERENCE_DATASETS = {"bic-lei", "isin-cusip", "mic-directory"}
PROFILE_ADDRESS_MODES = {"permissive", "require-structured", "forbid-unstructured"}
MAX_PROFILE_DER_BLOBS = 8
MAX_PROFILE_DER_BYTES = 1024 * 1024
MAX_PROFILE_DER_BASE64_CHARS = ((MAX_PROFILE_DER_BYTES + 2) // 3) * 4
MAX_MANIFEST_JSON_BYTES = 4 * 1024 * 1024
MAX_PROFILE_CATALOG_BYTES = 4 * 1024 * 1024
MAX_SCHEMA_BYTES = 8 * 1024 * 1024
MAX_FIXTURE_XML_BYTES = 8 * 1024 * 1024
MAX_XMLLINT_OUTPUT_BYTES = 64 * 1024
MAX_SOURCE_REPOSITORY_CHARS = 2048
MAX_REVIEWED_GAP_REASON_CHARS = 1024
MAX_XML_IDENTIFIER_CHARS = 256
MAX_PROFILE_CATALOG_IDENTIFIER_CHARS = 128
DEFAULT_XMLLINT_TIMEOUT_SECS = 30.0
ALLOWED_SCHEMA_SOURCE_LICENSES = {"Apache-2.0"}
DEFAULT_MANIFEST = (
    Path(__file__).resolve().parents[1]
    / "fixtures"
    / "iso20022"
    / "xsd"
    / "fixture_manifest.json"
)
DEFAULT_PROFILE_CATALOG = (
    Path(__file__).resolve().parents[1]
    / "crates"
    / "iroha_core"
    / "src"
    / "iso_bridge"
    / "profiles.rs"
)
PROFILE_CATALOG_RE = re.compile(
    r'^[ \t]*(?:pub(?:\s*\([^)\n]*\))?\s+)?const\s+DEFAULT_PROFILES_JSON\s*'
    r':\s*&str\s*=\s*r(?P<hashes>#*)"(?P<body>.*?)"(?P=hashes)\s*;',
    re.M | re.S,
)
RESTRICTED_SCHEMA_TEXT_MARKERS = (
    "may only be redistributed upon agreement",
    "no right, or right to authorise others",
    "rent, lease, or sell this component",
    "display publicly, distribute or otherwise provide this component",
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

TOP_LEVEL_KEYS = {"version", "schemas", "fixtures", "blocked_schema_sources"}
SCHEMA_KEYS = {"path", "message_def_id", "payload_root", "source", "schema_only_reason"}
SCHEMA_SOURCE_KEYS = {"repository", "commit", "path", "license", "sha256"}
BLOCKED_SCHEMA_SOURCE_KEYS = {
    "message_def_id",
    "source",
    "reason",
    "restriction_markers",
}
BLOCKED_SCHEMA_SOURCE_PROVENANCE_KEYS = {"repository", "commit", "path", "sha256"}
BLOCKED_SCHEMA_RESTRICTION_MARKERS = {
    "swift-copyright-header",
    "licensed-product-redistribution-agreement",
    "no-public-distribution-right",
    "exclusive-swift-property",
}
BLOCKED_SCHEMA_DISTRIBUTION_RESTRICTION_MARKERS = (
    BLOCKED_SCHEMA_RESTRICTION_MARKERS - {"swift-copyright-header"}
)
FIXTURE_KEYS = {
    "path",
    "message_def_id",
    "payload_root",
    "schema",
    "missing_schema_reason",
}
PROFILE_CATALOG_PROFILE_KEYS = {
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
    "required_reference_datasets",
    "message_profiles",
}
PROFILE_CATALOG_MESSAGE_KEYS = {
    "message_type",
    "direction",
    "versions",
    "business_services",
    "require_app_header",
    "require_business_service",
    "require_uetr",
    "structured_address_mode",
    "supplementary_data_max_bytes",
    "amount_minor_units",
}


class FixtureManifestError(RuntimeError):
    """Raised when ISO XSD fixture manifest wiring is malformed or incomplete."""


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
        raise FixtureManifestError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    mode = metadata.st_mode
    if stat.S_ISLNK(mode):
        raise FixtureManifestError(f"{path} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise FixtureManifestError(f"{path} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise FixtureManifestError(f"{path} exceeds {max_bytes} byte input limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise FixtureManifestError(f"{path} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise FixtureManifestError(f"{path} exceeds {max_bytes} byte input limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise FixtureManifestError(f"{path} exceeds {max_bytes} byte input limit")
        return raw
    except FileNotFoundError as error:
        raise FixtureManifestError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise FixtureManifestError(f"{path} must not be a symlink") from error
        raise FixtureManifestError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise FixtureManifestError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise FixtureManifestError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise FixtureManifestError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise FixtureManifestError(f"{label} must use forward slashes")
    if ";" in raw:
        raise FixtureManifestError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise FixtureManifestError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise FixtureManifestError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise FixtureManifestError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise FixtureManifestError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise FixtureManifestError(f"{label} must be a non-empty path")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise FixtureManifestError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise FixtureManifestError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise FixtureManifestError(f"{label} must use forward slashes")
    if ";" in raw:
        raise FixtureManifestError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise FixtureManifestError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise FixtureManifestError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise FixtureManifestError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise FixtureManifestError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise FixtureManifestError(f"{label} must not contain dot or parent segments")


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise FixtureManifestError(
                f"{label} must not contain malformed percent escapes"
            )
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise FixtureManifestError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise FixtureManifestError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise FixtureManifestError(
                f"{label} must not contain encoded semicolon parameters"
            )
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise FixtureManifestError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise FixtureManifestError(
                f"{label} must not contain encoded percent characters"
            )
        index += 3


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise FixtureManifestError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in arg):
            raise FixtureManifestError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise FixtureManifestError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise FixtureManifestError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise FixtureManifestError("argument terminator is not supported")
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise FixtureManifestError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise FixtureManifestError(f"{arg} does not take a value")
        index += 1


def _preflight_output_cli_paths(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise FixtureManifestError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise FixtureManifestError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise FixtureManifestError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise FixtureManifestError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


def _reject_raw_numeric_cli_value(raw: str, flag: str, *, integer: bool) -> None:
    if raw != raw.strip() or any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{flag} must be a numeric value")
    if any(ord(ch) > 0x7E for ch in raw):
        raise FixtureManifestError(f"{flag} must use printable ASCII")
    try:
        int(raw, 10) if integer else float(raw)
    except ValueError as error:
        raise FixtureManifestError(f"{flag} must be a numeric value") from error


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
            raise FixtureManifestError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise FixtureManifestError(f"{flag} requires a numeric value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise FixtureManifestError(f"{flag} requires a numeric value")
                _reject_raw_numeric_cli_value(value, flag, integer=flag in integer_flags)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise FixtureManifestError(f"{flag} requires a numeric value")
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
        raise FixtureManifestError(f"{path.parent} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise FixtureManifestError(f"{path.parent} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise FixtureManifestError(f"{path.parent} must be a directory")
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise FixtureManifestError(f"{path} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise FixtureManifestError(f"{path} must be a regular file")
        if metadata.st_nlink > 1:
            raise FixtureManifestError(f"{path} must not be hard-linked")
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise FixtureManifestError(f"{path.parent} must not be a symlink") from error
        raise FixtureManifestError(f"{path.parent} must be a directory") from error

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
                raise FixtureManifestError(
                    f"{path} temp file must not be a symlink"
                ) from error
            raise FixtureManifestError(
                f"cannot open temporary output for {path}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise FixtureManifestError(f"{path} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise FixtureManifestError(f"{path} temp file must not be hard-linked")
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
            raise FixtureManifestError(f"{current} must not be a symlink")


def _load_json(path: Path) -> Any:
    return _load_json_bytes(
        _read_regular_file(path, max_bytes=MAX_MANIFEST_JSON_BYTES),
        path,
    )


def _load_json_bytes(raw: bytes, path: Path) -> Any:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise FixtureManifestError(f"{path} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid JSON: {error}") from error
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
        raise FixtureManifestError("output limit bytes must be positive")
    timeout_secs = _require_positive_finite_number(timeout_secs, "xmllint timeout seconds")
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


def _require_positive_finite_number(value: Any, label: str) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise FixtureManifestError(f"{label} must be a positive finite number")
    parsed = float(value)
    if not math.isfinite(parsed) or parsed <= 0:
        raise FixtureManifestError(f"{label} must be a positive finite number")
    return parsed


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise FixtureManifestError("duplicate key in JSON object")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise FixtureManifestError(f"JSON contains non-finite numeric constant {value}")


def _reject_json_surrogates(value: Any) -> None:
    if isinstance(value, str):
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise FixtureManifestError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        for item in value:
            _reject_json_surrogates(item)
    elif isinstance(value, dict):
        for key, item in value.items():
            _reject_json_surrogates(key)
            _reject_json_surrogates(item)


def _rust_raw_string_end(text: str, start: int) -> int | None:
    if start >= len(text) or text[start] != "r":
        return None
    cursor = start + 1
    while cursor < len(text) and text[cursor] == "#":
        cursor += 1
    if cursor >= len(text) or text[cursor] != '"':
        return None
    delimiter = '"' + ("#" * (cursor - start - 1))
    end = text.find(delimiter, cursor + 1)
    if end < 0:
        return len(text)
    return end + len(delimiter)


def _rust_offset_is_ignored_span(text: str, offset: int) -> bool:
    index = 0
    block_depth = 0
    line_comment = False
    while index < offset:
        if line_comment:
            if text[index] == "\n":
                line_comment = False
            index += 1
            continue
        if block_depth:
            if text.startswith("/*", index):
                block_depth += 1
                index += 2
                continue
            if text.startswith("*/", index):
                block_depth -= 1
                index += 2
                continue
            index += 1
            continue
        raw_end = _rust_raw_string_end(text, index)
        if raw_end is not None:
            if offset < raw_end:
                return True
            index = raw_end
            continue
        if text.startswith("//", index):
            line_comment = True
            index += 2
            continue
        if text.startswith("/*", index):
            block_depth = 1
            index += 2
            continue
        if text[index] == '"':
            quote = text[index]
            index += 1
            while index < len(text):
                if index >= offset:
                    return True
                if text[index] == "\\":
                    index += 2
                    continue
                if text[index] == quote:
                    index += 1
                    break
                index += 1
            continue
        index += 1
    return line_comment or bool(block_depth)


def _profile_catalog_match(text: str, path: Path) -> re.Match[str]:
    matches = [
        match
        for match in PROFILE_CATALOG_RE.finditer(text)
        if not _rust_offset_is_ignored_span(text, match.start())
    ]
    if not matches:
        raise FixtureManifestError(
            f"{path} does not contain DEFAULT_PROFILES_JSON raw string"
        )
    if len(matches) > 1:
        raise FixtureManifestError(
            f"{path} must contain exactly one DEFAULT_PROFILES_JSON raw string"
        )
    return matches[0]


def _load_profile_catalog(path: Path) -> tuple[list[Any], str, str]:
    raw = _read_regular_file(path, max_bytes=MAX_PROFILE_CATALOG_BYTES)
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid UTF-8") from error
    match = _profile_catalog_match(text, path)
    catalog_json = match.group("body")
    try:
        catalog = json.loads(
            catalog_json,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise FixtureManifestError(
            f"{path} DEFAULT_PROFILES_JSON is not valid JSON: {error}"
        ) from error
    _reject_json_surrogates(catalog)
    return (
        _require_array(catalog, f"{path}.DEFAULT_PROFILES_JSON"),
        sha256_hex(raw),
        sha256_hex(catalog_json.encode("utf-8")),
    )


def _reject_xml_dtd_or_entities(raw: bytes, path: Path) -> None:
    if b"<!DOCTYPE" in raw or b"<!ENTITY" in raw:
        raise FixtureManifestError(
            f"{path} must not contain DTD or entity declarations"
        )


def _parse_xml(path: Path) -> ET.Element:
    return _parse_xml_bytes(
        _read_regular_file(path, max_bytes=MAX_FIXTURE_XML_BYTES),
        path,
    )


def _parse_xml_bytes(raw: bytes, path: Path) -> ET.Element:
    _reject_xml_dtd_or_entities(raw, path)
    try:
        return ET.fromstring(raw)
    except ET.ParseError as error:
        raise FixtureManifestError(f"{path} is not well-formed XML: {error}") from error


def _reject_restricted_schema_terms(raw: bytes, path: Path) -> None:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise FixtureManifestError(f"{path} is not valid UTF-8") from error
    lowered = text.casefold()
    for marker in RESTRICTED_SCHEMA_TEXT_MARKERS:
        if marker in lowered:
            raise FixtureManifestError(
                f"{path} contains restricted redistribution terms; "
                "do not check in licensed Standards Editor packages without redistribution rights"
            )


def _reject_secret_looking_material(value: str, label: str) -> None:
    if _contains_secret_material(value):
        raise FixtureManifestError(f"{label} must not contain secret-looking material")


def _reject_secret_looking_path_material(value: str, label: str) -> None:
    if _contains_secret_material(value) or _contains_secret_identifier_material(value):
        raise FixtureManifestError(f"{label} must not contain secret-looking material")


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise FixtureManifestError(f"{label} must be a JSON object")
    return value


def _require_array(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise FixtureManifestError(f"{label} must be a JSON array")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        if any(
            _is_secret_looking_key(key)
            or _is_control_bearing_key(key)
            or len(str(key)) > 128
            or any(ord(ch) > 0x7E for ch in str(key))
            for key in unknown
        ) or len(unknown) > 8 or sum(len(str(key)) for key in unknown) > 256:
            raise FixtureManifestError(f"{label} contains unknown keys")
        raise FixtureManifestError(f"{label} contains unknown keys: {', '.join(unknown)}")


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


def _contains_unsafe_json_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\r", "\t"}) or ord(ch) == 0x7F
        for ch in value
    )


def _contains_non_ascii_material(value: str) -> bool:
    return any(ord(ch) > 0x7E for ch in value)


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise FixtureManifestError(f"{label} must not contain secret-looking material")


def _contains_overlong_xml_identifier(value: str) -> bool:
    return len(value) > MAX_XML_IDENTIFIER_CHARS


def _reject_overlong_xml_identifier(value: str, label: str) -> None:
    if _contains_overlong_xml_identifier(value):
        raise FixtureManifestError(
            f"{label} must be no longer than {MAX_XML_IDENTIFIER_CHARS} characters"
        )


def _reject_non_ascii_xml_identifiers(element: ET.Element, path: Path, label: str) -> None:
    if isinstance(element.tag, str):
        namespace, local = _split_xml_name(element.tag)
        if namespace is not None:
            _reject_overlong_xml_identifier(namespace, f"{path} {label} namespace")
            _reject_non_ascii_identifier(namespace, f"{path} {label} namespace")
        _reject_overlong_xml_identifier(local, f"{path} {label} element")
        _reject_non_ascii_identifier(local, f"{path} {label} element")
    for attr_name in element.attrib:
        _reject_overlong_xml_identifier(attr_name, f"{path} {label} attribute")
        _reject_non_ascii_identifier(attr_name, f"{path} {label} attribute")
    for offset, child in enumerate(element):
        _reject_non_ascii_xml_identifiers(child, path, f"{label}[{offset}]")


def _reject_secret_looking_xml_content(element: ET.Element, path: Path, label: str) -> None:
    if isinstance(element.tag, str):
        namespace, local = _split_xml_name(element.tag)
        if namespace is not None:
            _reject_secret_looking_identifier(namespace, f"{path} {label} namespace")
        _reject_secret_looking_identifier(local, f"{path} {label} element")
    for attr_name, attr_value in element.attrib.items():
        _reject_secret_looking_identifier(attr_name, f"{path} {label} attribute")
        _reject_secret_looking_material(attr_value, f"{path} {label} attribute value")
    if element.text:
        _reject_secret_looking_material(element.text, f"{path} {label} text")
    if element.tail:
        _reject_secret_looking_material(element.tail, f"{path} {label} tail")
    for offset, child in enumerate(element):
        _reject_secret_looking_xml_content(child, path, f"{label}[{offset}]")


def _check_no_secret_material(value: Any, label: str = "$") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if _is_secret_looking_key(str(key)):
                raise FixtureManifestError(f"{label} contains forbidden secret-looking field")
            if _is_control_bearing_key(key):
                raise FixtureManifestError(f"{label} contains forbidden control-bearing field")
            _check_no_secret_material(child, f"{label}.{key}")
    elif isinstance(value, list):
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{label}[{offset}]")
    elif isinstance(value, str):
        if _contains_unsafe_json_control(value):
            raise FixtureManifestError(f"{label} contains unsafe control characters")
        if _contains_secret_material(value) or _is_secret_looking_key(value):
            raise FixtureManifestError(f"{label} contains secret-looking material")


def _required_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise FixtureManifestError(f"{label}.{key} must be a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label}.{key} must not contain control characters")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _reject_non_ascii_identifier(value: str, label: str) -> None:
    if _contains_non_ascii_material(value):
        raise FixtureManifestError(f"{label} must use printable ASCII")


def _reject_reviewed_gap_reason_material(value: str, label: str) -> None:
    if len(value) > MAX_REVIEWED_GAP_REASON_CHARS:
        raise FixtureManifestError(
            f"{label} must be no longer than {MAX_REVIEWED_GAP_REASON_CHARS} characters"
        )
    _reject_non_ascii_identifier(value, label)
    _reject_secret_looking_identifier(value, label)


def _reject_overlong_profile_catalog_enum(value: str, label: str) -> None:
    if len(value) > MAX_PROFILE_CATALOG_IDENTIFIER_CHARS:
        raise FixtureManifestError(
            f"{label} must be no longer than {MAX_PROFILE_CATALOG_IDENTIFIER_CHARS} characters"
        )


def _reject_overlong_profile_catalog_identifier(value: str, label: str) -> None:
    if len(value) > MAX_PROFILE_CATALOG_IDENTIFIER_CHARS:
        raise FixtureManifestError(
            f"{label} must be no longer than {MAX_PROFILE_CATALOG_IDENTIFIER_CHARS} characters"
        )


def _require_message_def_id(value: str, label: str) -> str:
    _reject_non_ascii_identifier(value, label)
    _reject_secret_looking_identifier(value, label)
    if MESSAGE_DEF_ID_RE.fullmatch(value) is None:
        raise FixtureManifestError(
            f"{label} must be lowercase ISO message id like pacs.008.001.08"
        )
    return value


def _optional_string(value: dict[str, Any], key: str, label: str) -> str | None:
    if key not in value:
        return None
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise FixtureManifestError(f"{label}.{key} must be a non-empty string when set")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label}.{key} must not contain control characters")
    if raw != raw.strip():
        raise FixtureManifestError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _optional_bool(value: dict[str, Any], key: str, label: str) -> bool | None:
    if key not in value:
        return None
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise FixtureManifestError(f"{label}.{key} must be a boolean when set")
    return raw


def _optional_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int | None:
    if key not in value:
        return None
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
        raise FixtureManifestError(f"{label}.{key} must be a non-negative integer when set")
    return raw


def _optional_string_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    if key not in value:
        return []
    raw = value.get(key)
    items = _require_array(raw, f"{label}.{key}")
    result: list[str] = []
    seen: dict[str, int] = {}
    for offset, item in enumerate(items):
        if not isinstance(item, str) or not item.strip():
            raise FixtureManifestError(f"{label}.{key}[{offset}] must be a non-empty string")
        if item != item.strip():
            raise FixtureManifestError(f"{label}.{key}[{offset}] must not have surrounding whitespace")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise FixtureManifestError(f"{label}.{key}[{offset}] must not contain control characters")
        _reject_non_ascii_identifier(item, f"{label}.{key}[{offset}]")
        if item in seen:
            raise FixtureManifestError(
                f"{label}.{key}[{offset}] duplicates {label}.{key}[{seen[item]}]"
            )
        seen[item] = offset
        result.append(item)
    return result


def _optional_sha256_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _optional_string_list(value, key, label)
    for offset, item in enumerate(items):
        if not _is_lower_sha256(item) or set(item) == {"0"}:
            raise FixtureManifestError(
                f"{label}.{key}[{offset}] must be a canonical nonzero SHA-256"
            )
    return items


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


def _optional_oid_list(value: dict[str, Any], key: str, label: str) -> list[str]:
    items = _optional_string_list(value, key, label)
    for offset, item in enumerate(items):
        if not _valid_oid(item):
            raise FixtureManifestError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
    return items


def _validate_optional_clean_string_list(
    value: dict[str, Any],
    key: str,
    label: str,
) -> list[str]:
    return _optional_string_list(value, key, label)


def _optional_canonical_base64_list(
    value: dict[str, Any],
    key: str,
    label: str,
) -> list[str]:
    items = _optional_string_list(value, key, label)
    if len(items) > MAX_PROFILE_DER_BLOBS:
        raise FixtureManifestError(
            f"{label}.{key} must not contain more than {MAX_PROFILE_DER_BLOBS} entries"
        )
    for offset, item in enumerate(items):
        if len(item) > MAX_PROFILE_DER_BASE64_CHARS:
            raise FixtureManifestError(
                f"{label}.{key}[{offset}] must decode to no more than "
                f"{MAX_PROFILE_DER_BYTES} bytes"
            )
        try:
            decoded = base64.b64decode(item, validate=True)
        except (ValueError, binascii.Error) as error:
            raise FixtureManifestError(f"{label}.{key}[{offset}] must be canonical base64") from error
        if not decoded:
            raise FixtureManifestError(f"{label}.{key}[{offset}] must be non-empty base64")
        if len(decoded) > MAX_PROFILE_DER_BYTES:
            raise FixtureManifestError(
                f"{label}.{key}[{offset}] must decode to no more than "
                f"{MAX_PROFILE_DER_BYTES} bytes"
            )
        _require_der_sequence(decoded, f"{label}.{key}[{offset}]")
        if base64.b64encode(decoded).decode("ascii") != item:
            raise FixtureManifestError(f"{label}.{key}[{offset}] must be canonical padded base64")
    return items


def _require_der_sequence(value: bytes, label: str) -> None:
    if not value or value[0] != 0x30:
        raise FixtureManifestError(f"{label} must be a DER SEQUENCE")
    if len(value) < 2:
        raise FixtureManifestError(f"{label} has truncated DER length")

    first_length = value[1]
    header_len = 2
    if first_length < 0x80:
        content_len = first_length
    else:
        length_octets = first_length & 0x7F
        if length_octets == 0 or length_octets > 4:
            raise FixtureManifestError(f"{label} has invalid DER length")
        if len(value) < 2 + length_octets:
            raise FixtureManifestError(f"{label} has truncated DER length")
        length_bytes = value[2 : 2 + length_octets]
        if length_bytes[0] == 0:
            raise FixtureManifestError(f"{label} has non-minimal DER length")
        content_len = int.from_bytes(length_bytes, "big")
        if content_len < 0x80:
            raise FixtureManifestError(f"{label} has non-minimal DER length")
        header_len += length_octets

    if header_len + content_len != len(value):
        raise FixtureManifestError(
            f"{label} DER length does not consume the whole value"
        )


def _reject_sha256_overlap(first: list[str], second: list[str], label: str) -> None:
    overlap = sorted(set(first) & set(second))
    if overlap:
        raise FixtureManifestError(f"{label} contains overlapping SHA-256 pin {overlap[0]}")


def _validate_profile_catalog_profile_fields(profile: dict[str, Any], label: str) -> None:
    rail = _required_string(profile, "rail", label)
    _reject_non_ascii_identifier(rail, f"{label}.rail")
    _reject_overlong_profile_catalog_enum(rail, f"{label}.rail")
    if rail not in PROFILE_RAILS:
        raise FixtureManifestError(f"{label}.rail has unknown rail {rail!r}")
    policy = _required_string(profile, "embedded_signature_policy", label)
    _reject_non_ascii_identifier(policy, f"{label}.embedded_signature_policy")
    _reject_overlong_profile_catalog_enum(
        policy,
        f"{label}.embedded_signature_policy",
    )
    if policy not in PROFILE_SIGNATURE_POLICIES:
        raise FixtureManifestError(
            f"{label}.embedded_signature_policy has unknown policy {policy!r}"
        )
    public_pins = _optional_sha256_list(profile, "signature_public_key_sha256_pins", label)
    legacy_public_pins = _optional_sha256_list(profile, "trusted_public_key_sha256", label)
    _reject_sha256_overlap(
        public_pins,
        legacy_public_pins,
        f"{label}.signature_public_key_sha256_pins/trusted_public_key_sha256",
    )
    anchor_pins = _optional_sha256_list(profile, "x509_trust_anchor_sha256_pins", label)
    legacy_anchor_pins = _optional_sha256_list(profile, "trusted_certificate_sha256", label)
    _reject_sha256_overlap(
        anchor_pins,
        legacy_anchor_pins,
        f"{label}.x509_trust_anchor_sha256_pins/trusted_certificate_sha256",
    )
    revoked_pins = _optional_sha256_list(profile, "revoked_certificate_sha256", label)
    _reject_sha256_overlap(
        anchor_pins + legacy_anchor_pins,
        revoked_pins,
        f"{label}.trusted/revoked certificate pins",
    )
    _optional_oid_list(profile, "x509_required_certificate_policy_oids", label)
    crl_required = _optional_bool(profile, "x509_require_crl_revocation_check", label)
    ocsp_required = _optional_bool(profile, "x509_require_ocsp_revocation_check", label)
    crls = _optional_canonical_base64_list(profile, "x509_crl_der_base64", label)
    ocsp_responses = _optional_canonical_base64_list(
        profile,
        "x509_ocsp_response_der_base64",
        label,
    )
    if crl_required and not crls:
        raise FixtureManifestError(
            f"{label}.x509_crl_der_base64 must not be empty when CRL revocation is required"
        )
    if ocsp_required and not ocsp_responses:
        raise FixtureManifestError(
            f"{label}.x509_ocsp_response_der_base64 must not be empty when OCSP revocation is required"
        )
    if policy == "require-verified" and not (public_pins or legacy_public_pins or anchor_pins or legacy_anchor_pins):
        raise FixtureManifestError(
            f"{label} uses require-verified but has no public-key or X.509 trust pins"
        )
    for offset, dataset in enumerate(
        _optional_string_list(profile, "required_reference_datasets", label)
    ):
        _reject_overlong_profile_catalog_enum(
            dataset,
            f"{label}.required_reference_datasets[{offset}]",
        )
        if dataset not in PROFILE_REFERENCE_DATASETS:
            raise FixtureManifestError(
                f"{label}.required_reference_datasets[{offset}] has unknown dataset {dataset!r}"
            )


def _validate_amount_minor_units(message: dict[str, Any], label: str) -> None:
    if "amount_minor_units" not in message:
        return
    raw = message.get("amount_minor_units")
    entries = _require_array(raw, f"{label}.amount_minor_units")
    seen: dict[str, int] = {}
    for offset, raw_entry in enumerate(entries):
        entry_label = f"{label}.amount_minor_units[{offset}]"
        entry = _require_object(raw_entry, entry_label)
        _reject_unknown_keys(entry, {"currency", "minor_units"}, entry_label)
        currency = _required_string(entry, "currency", entry_label)
        if PROFILE_CURRENCY_RE.fullmatch(currency) is None:
            raise FixtureManifestError(f"{entry_label}.currency must be an uppercase ISO 4217 code")
        if currency in seen:
            raise FixtureManifestError(
                f"{entry_label}.currency duplicates "
                f"{label}.amount_minor_units[{seen[currency]}].currency"
            )
        seen[currency] = offset
        units = _optional_nonnegative_int(entry, "minor_units", entry_label)
        if units is None:
            raise FixtureManifestError(f"{entry_label}.minor_units must be set")
        if units > 255:
            raise FixtureManifestError(f"{entry_label}.minor_units must fit in u8")


def _validate_profile_catalog_message_fields(message: dict[str, Any], label: str) -> None:
    business_services = _optional_string_list(message, "business_services", label)
    for offset, service in enumerate(business_services):
        _reject_overlong_profile_catalog_identifier(
            service,
            f"{label}.business_services[{offset}]",
        )
    require_app_header = _optional_bool(message, "require_app_header", label)
    require_business_service = _optional_bool(message, "require_business_service", label)
    _optional_bool(message, "require_uetr", label)
    address_mode = _required_string(message, "structured_address_mode", label)
    _reject_non_ascii_identifier(address_mode, f"{label}.structured_address_mode")
    _reject_overlong_profile_catalog_enum(
        address_mode,
        f"{label}.structured_address_mode",
    )
    if address_mode not in PROFILE_ADDRESS_MODES:
        raise FixtureManifestError(
            f"{label}.structured_address_mode has unknown mode {address_mode!r}"
        )
    supplementary_data_max_bytes = _optional_nonnegative_int(
        message,
        "supplementary_data_max_bytes",
        label,
    )
    if supplementary_data_max_bytes == 0:
        raise FixtureManifestError(f"{label}.supplementary_data_max_bytes must be positive")
    if require_business_service and not business_services:
        raise FixtureManifestError(
            f"{label}.business_services must not be empty when require_business_service is true"
        )
    if require_business_service and require_app_header is False:
        raise FixtureManifestError(
            f"{label}.require_app_header must be true when require_business_service is true"
        )
    _validate_amount_minor_units(message, label)


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _required_sha256(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not _is_lower_sha256(raw):
        raise FixtureManifestError(f"{label}.{key} must be a lowercase SHA-256 digest")
    return raw


def _validate_source_path(raw: str, label: str) -> str:
    if "\\" in raw:
        raise FixtureManifestError(f"{label} must use forward slashes")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label} must not contain control characters")
    if any(ch.isspace() for ch in raw):
        raise FixtureManifestError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise FixtureManifestError(f"{label} must not start with a dash")
    if ";" in raw:
        raise FixtureManifestError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise FixtureManifestError(f"{label} must not contain URI or drive prefixes")
    _reject_percent_encoded_path_smuggling(raw, label)
    if any(part.startswith("-") for part in raw.split("/") if part):
        raise FixtureManifestError(f"{label} must not contain leading-dash path segments")
    path = Path(raw)
    if path.is_absolute():
        raise FixtureManifestError(f"{label} must be relative")
    if not raw.endswith(".xsd"):
        raise FixtureManifestError(f"{label} must point to an .xsd file")
    if any(part in {"", ".", ".."} for part in path.parts):
        raise FixtureManifestError(f"{label} must not contain empty, dot, or parent segments")
    _reject_secret_looking_path_material(raw, label)
    return raw


def _source_repository_component_is_placeholder(component: str) -> bool:
    lowered = component.casefold()
    if lowered in PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS:
        return True
    if lowered.endswith(".example"):
        return True
    return any(
        token in PLACEHOLDER_SOURCE_REPOSITORY_COMPONENTS
        for token in re.split(r"[-_.]+", lowered)
        if token
    )


def _validate_source_repository(raw: str, label: str) -> str:
    if len(raw) > MAX_SOURCE_REPOSITORY_CHARS:
        raise FixtureManifestError(
            f"{label} must be no longer than {MAX_SOURCE_REPOSITORY_CHARS} characters"
        )
    if SOURCE_REPOSITORY_RE.fullmatch(raw) is None or raw.endswith(".git"):
        raise FixtureManifestError(
            f"{label} must be a canonical https://github.com/<org>/<repo> URL"
        )
    _reject_secret_looking_identifier(raw, label)
    repository_parts = urllib.parse.urlparse(raw).path.strip("/").split("/")
    if len(repository_parts) != 2:
        raise FixtureManifestError(
            f"{label} must be a canonical https://github.com/<org>/<repo> URL"
        )
    if any(_source_repository_component_is_placeholder(part) for part in repository_parts):
        raise FixtureManifestError(f"{label} must not use placeholder repository coordinates")
    return raw


def _verify_schema_source(
    value: Any,
    label: str,
    *,
    message_def_id: str,
    schema_sha256: str,
) -> dict[str, str]:
    source = _require_object(value, label)
    _reject_unknown_keys(source, SCHEMA_SOURCE_KEYS, label)
    repository = _validate_source_repository(
        _required_string(source, "repository", label),
        f"{label}.repository",
    )
    commit = _required_string(source, "commit", label)
    if SOURCE_COMMIT_RE.fullmatch(commit) is None:
        raise FixtureManifestError(f"{label}.commit must be a lowercase 40-hex Git commit")
    source_path = _validate_source_path(_required_string(source, "path", label), f"{label}.path")
    if Path(source_path).name != f"{message_def_id}.xsd":
        raise FixtureManifestError(
            f"{label}.path filename must match message_def_id {message_def_id!r}"
        )
    license_id = _required_string(source, "license", label)
    if license_id not in ALLOWED_SCHEMA_SOURCE_LICENSES:
        raise FixtureManifestError(
            f"{label}.license must be one of "
            + ", ".join(sorted(ALLOWED_SCHEMA_SOURCE_LICENSES))
        )
    source_sha256 = _required_sha256(source, "sha256", label)
    if source_sha256 != schema_sha256:
        raise FixtureManifestError(
            f"{label}.sha256 does not match checked-in XSD bytes"
        )
    return {
        "repository": repository,
        "commit": commit,
        "path": source_path,
        "license": license_id,
        "sha256": source_sha256,
    }


def _verify_blocked_schema_source(value: Any, label: str) -> dict[str, Any]:
    entry = _require_object(value, label)
    _reject_unknown_keys(entry, BLOCKED_SCHEMA_SOURCE_KEYS, label)
    message_def_id = _require_message_def_id(
        _required_string(entry, "message_def_id", label),
        f"{label}.message_def_id",
    )
    reason = _required_string(entry, "reason", label)
    _reject_reviewed_gap_reason_material(reason, f"{label}.reason")
    if "source" not in entry:
        raise FixtureManifestError(f"{label}.source must be recorded")
    source = _require_object(entry["source"], f"{label}.source")
    _reject_unknown_keys(
        source,
        BLOCKED_SCHEMA_SOURCE_PROVENANCE_KEYS,
        f"{label}.source",
    )
    repository = _required_string(source, "repository", f"{label}.source")
    repository = _validate_source_repository(repository, f"{label}.source.repository")
    commit = _required_string(source, "commit", f"{label}.source")
    if SOURCE_COMMIT_RE.fullmatch(commit) is None:
        raise FixtureManifestError(f"{label}.source.commit must be a lowercase 40-hex Git commit")
    source_path = _validate_source_path(
        _required_string(source, "path", f"{label}.source"),
        f"{label}.source.path",
    )
    if Path(source_path).name != f"{message_def_id}.xsd":
        raise FixtureManifestError(
            f"{label}.source.path filename must match message_def_id {message_def_id!r}"
        )
    source_sha256 = _required_sha256(source, "sha256", f"{label}.source")

    raw_markers = _require_array(
        entry.get("restriction_markers"),
        f"{label}.restriction_markers",
    )
    if not raw_markers:
        raise FixtureManifestError(f"{label}.restriction_markers must not be empty")
    markers: list[str] = []
    seen_markers: dict[str, int] = {}
    for offset, raw_marker in enumerate(raw_markers):
        marker_label = f"{label}.restriction_markers[{offset}]"
        if not isinstance(raw_marker, str) or not raw_marker.strip():
            raise FixtureManifestError(f"{marker_label} must be a non-empty string")
        if raw_marker != raw_marker.strip():
            raise FixtureManifestError(f"{marker_label} must not have surrounding whitespace")
        if raw_marker not in BLOCKED_SCHEMA_RESTRICTION_MARKERS:
            raise FixtureManifestError(
                f"{marker_label} must be one of "
                + ", ".join(sorted(BLOCKED_SCHEMA_RESTRICTION_MARKERS))
            )
        if raw_marker in seen_markers:
            raise FixtureManifestError(
                f"{marker_label} duplicates {label}.restriction_markers[{seen_markers[raw_marker]}]"
            )
        seen_markers[raw_marker] = offset
        markers.append(raw_marker)
    if not (
        set(markers) & BLOCKED_SCHEMA_DISTRIBUTION_RESTRICTION_MARKERS
    ):
        raise FixtureManifestError(
            f"{label}.restriction_markers must include a redistribution restriction marker"
        )

    return {
        "message_def_id": message_def_id,
        "source": {
            "repository": repository,
            "commit": commit,
            "path": source_path,
            "sha256": source_sha256,
        },
        "reason": reason,
        "restriction_markers": markers,
    }


def _split_xml_name(name: str) -> tuple[str | None, str]:
    if name.startswith("{"):
        namespace, local = name[1:].split("}", 1)
        return namespace, local
    return None, name


def _namespace_for(message_def_id: str) -> str:
    return ISO_NAMESPACE_PREFIX + message_def_id


def _message_id_from_namespace(namespace: str | None, label: str) -> str:
    if namespace is None:
        raise FixtureManifestError(f"{label} namespace must start with {ISO_NAMESPACE_PREFIX}")
    _reject_overlong_xml_identifier(namespace, f"{label} namespace")
    _reject_non_ascii_identifier(namespace, f"{label} namespace")
    if not namespace.startswith(ISO_NAMESPACE_PREFIX):
        raise FixtureManifestError(f"{label} namespace must start with {ISO_NAMESPACE_PREFIX}")
    value = namespace[len(ISO_NAMESPACE_PREFIX) :]
    if not value:
        raise FixtureManifestError(f"{label} namespace has empty message definition id")
    return _require_message_def_id(value, f"{label} namespace message definition id")


def _schema_children(parent: ET.Element, local_name: str, **attrs: str) -> list[ET.Element]:
    result: list[ET.Element] = []
    for child in parent:
        if not isinstance(child.tag, str):
            continue
        namespace, local = _split_xml_name(child.tag)
        if (
            namespace == XML_SCHEMA_NS
            and local == local_name
            and all(child.attrib.get(key) == value for key, value in attrs.items())
        ):
            result.append(child)
    return result


def _schema_child_locals(parent: ET.Element, path: Path, label: str) -> list[str]:
    locals_: list[str] = []
    for child in parent:
        if not isinstance(child.tag, str):
            continue
        namespace, local = _split_xml_name(child.tag)
        if namespace != XML_SCHEMA_NS:
            raise FixtureManifestError(f"{path} {label} contains unsupported child")
        locals_.append(local)
    return locals_


def _reject_unsupported_schema_composition(root: ET.Element, path: Path) -> None:
    for child in root:
        if not isinstance(child.tag, str):
            continue
        namespace, local = _split_xml_name(child.tag)
        if namespace != XML_SCHEMA_NS:
            raise FixtureManifestError(
                f"{path} xs:schema contains unsupported foreign child"
            )
        if local in UNSUPPORTED_SCHEMA_COMPOSITION_CHILDREN:
            raise FixtureManifestError(f"{path} xs:schema must not contain xs:{local}")


def _require_schema_attributes(
    element: ET.Element,
    expected: set[str],
    path: Path,
    label: str,
) -> None:
    actual = set(element.attrib)
    if actual != expected:
        missing = sorted(expected - actual)
        extra = sorted(actual - expected)
        details = []
        if missing:
            details.append("missing " + ", ".join(missing))
        if extra:
            if any(
                _contains_secret_material(name)
                or _is_secret_looking_key(name)
                or _contains_non_ascii_material(name)
                or _contains_overlong_xml_identifier(name)
                for name in extra
            ):
                details.append("unexpected attributes")
            else:
                details.append("unexpected " + ", ".join(extra))
        suffix = ": " + "; ".join(details) if details else ""
        raise FixtureManifestError(
            f"{path} {label} must declare exactly {', '.join(sorted(expected))}{suffix}"
        )


def _schema_payload_root(root: ET.Element, path: Path) -> str:
    document_elements = _schema_children(root, "element", name="Document")
    if not document_elements:
        raise FixtureManifestError(f"{path} has no top-level xs:element name='Document'")
    if len(document_elements) != 1:
        raise FixtureManifestError(
            f"{path} must contain exactly one top-level xs:element name='Document'"
        )
    document_element = document_elements[0]
    _require_schema_attributes(
        document_element,
        {"name", "type"},
        path,
        "Document element",
    )
    document_type = document_element.attrib.get("type")
    if not document_type:
        raise FixtureManifestError(f"{path} Document element does not declare a type")
    _reject_overlong_xml_identifier(document_type, f"{path} Document element type")
    _reject_non_ascii_identifier(document_type, f"{path} Document element type")
    _reject_secret_looking_identifier(document_type, f"{path} Document element type")
    if document_type != "Document":
        raise FixtureManifestError(
            f"{path} Document element type must be exactly 'Document'"
        )
    document_complexes = _schema_children(root, "complexType", name=document_type)
    if not document_complexes:
        raise FixtureManifestError(f"{path} has no xs:complexType name={document_type!r}")
    if len(document_complexes) != 1:
        raise FixtureManifestError(
            f"{path} must contain exactly one xs:complexType name={document_type!r}"
        )
    document_complex = document_complexes[0]
    if _schema_child_locals(document_complex, path, "Document complex type") != [
        "sequence"
    ]:
        raise FixtureManifestError(
            f"{path} Document complex type must contain only one direct xs:sequence"
        )
    sequences = _schema_children(document_complex, "sequence")
    if not sequences:
        raise FixtureManifestError(f"{path} Document complex type has no xs:sequence")
    if len(sequences) != 1:
        raise FixtureManifestError(
            f"{path} Document complex type must contain exactly one xs:sequence"
        )
    sequence = sequences[0]
    payload_elements = _schema_children(sequence, "element")
    if len(payload_elements) != 1:
        raise FixtureManifestError(
            f"{path} Document sequence must contain exactly one payload element"
        )
    if _schema_child_locals(sequence, path, "Document sequence") != ["element"]:
        raise FixtureManifestError(
            f"{path} Document sequence must contain only one direct xs:element"
        )
    payload_element = payload_elements[0]
    _require_schema_attributes(
        payload_element,
        {"name", "type"},
        path,
        "Document payload element",
    )
    payload = payload_element.attrib.get("name")
    if not payload:
        raise FixtureManifestError(f"{path} Document payload element has no name")
    _reject_overlong_xml_identifier(payload, f"{path} Document payload element name")
    _reject_non_ascii_identifier(payload, f"{path} Document payload element name")
    _reject_secret_looking_identifier(payload, f"{path} Document payload element name")
    payload_type = payload_element.attrib.get("type")
    if not payload_type:
        raise FixtureManifestError(f"{path} Document payload element does not declare a type")
    _reject_overlong_xml_identifier(payload_type, f"{path} Document payload element type")
    _reject_non_ascii_identifier(payload_type, f"{path} Document payload element type")
    _reject_secret_looking_identifier(payload_type, f"{path} Document payload element type")
    if ":" in payload_type:
        raise FixtureManifestError(
            f"{path} Document payload element type must be local and unprefixed"
        )
    payload_complexes = _schema_children(root, "complexType", name=payload_type)
    if not payload_complexes:
        raise FixtureManifestError(
            f"{path} has no xs:complexType name={payload_type!r}"
        )
    if len(payload_complexes) != 1:
        raise FixtureManifestError(
            f"{path} must contain exactly one xs:complexType name={payload_type!r}"
        )
    payload_complex = payload_complexes[0]
    if _schema_child_locals(payload_complex, path, f"payload complex type {payload_type!r}") != [
        "sequence"
    ]:
        raise FixtureManifestError(
            f"{path} payload complex type {payload_type!r} must contain only one direct xs:sequence"
        )
    return payload


def _validate_relative_path(
    raw: str,
    base: Path,
    containment_root: Path,
    label: str,
    *,
    allow_parent_segments: bool,
) -> Path:
    if "\\" in raw:
        raise FixtureManifestError(f"{label} must use forward slashes")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise FixtureManifestError(f"{label} must not contain control characters")
    if any(ch.isspace() for ch in raw):
        raise FixtureManifestError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise FixtureManifestError(f"{label} must not start with a dash")
    if ";" in raw:
        raise FixtureManifestError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise FixtureManifestError(f"{label} must not contain URI or drive prefixes")
    _reject_percent_encoded_path_smuggling(raw, label)
    if any(part.startswith("-") for part in raw.split("/") if part):
        raise FixtureManifestError(f"{label} must not contain leading-dash path segments")
    path = Path(raw)
    if path.is_absolute():
        raise FixtureManifestError(f"{label} must be relative")
    parts = raw.split("/")
    if any(part in {"", "."} for part in parts):
        raise FixtureManifestError(f"{label} must not contain empty or dot segments")
    if not allow_parent_segments and ".." in parts:
        raise FixtureManifestError(f"{label} must not contain parent segments")
    seen_child_segment = False
    for part in parts:
        if part == "..":
            if seen_child_segment:
                raise FixtureManifestError(f"{label} parent segments must be leading")
        else:
            seen_child_segment = True
    candidate = base / path
    resolved_parent = candidate.parent.resolve()
    root = containment_root.resolve()
    if not resolved_parent.is_relative_to(root):
        raise FixtureManifestError(f"{label} must stay under {root}")
    _reject_secret_looking_path_material(raw, label)
    return candidate


def verify_schema_entry(
    entry: dict[str, Any],
    label: str,
    manifest_dir: Path,
) -> dict[str, Any]:
    """Verify one schema manifest entry and return normalized metadata."""

    _reject_unknown_keys(entry, SCHEMA_KEYS, label)
    rel_path = _required_string(entry, "path", label)
    message_def_id = _require_message_def_id(
        _required_string(entry, "message_def_id", label),
        f"{label}.message_def_id",
    )
    expected_payload_root = _required_string(entry, "payload_root", label)
    _reject_overlong_xml_identifier(expected_payload_root, f"{label}.payload_root")
    _reject_non_ascii_identifier(expected_payload_root, f"{label}.payload_root")
    _reject_secret_looking_identifier(expected_payload_root, f"{label}.payload_root")
    schema_only_reason = _optional_string(entry, "schema_only_reason", label)
    if schema_only_reason is not None:
        _reject_reviewed_gap_reason_material(
            schema_only_reason,
            f"{label}.schema_only_reason",
        )
    if not rel_path.endswith(".xsd"):
        raise FixtureManifestError(f"{label}.path must point to an .xsd file")
    path = _validate_relative_path(
        rel_path,
        manifest_dir,
        manifest_dir,
        f"{label}.path",
        allow_parent_segments=False,
    )
    if Path(rel_path).stem != message_def_id:
        raise FixtureManifestError(f"{label}.path stem must equal message_def_id")
    schema_bytes = _read_regular_file(path, max_bytes=MAX_SCHEMA_BYTES)
    _reject_restricted_schema_terms(schema_bytes, path)
    schema_sha256 = sha256_hex(schema_bytes)
    if "source" not in entry:
        raise FixtureManifestError(f"{label}.source must be recorded")
    source = _verify_schema_source(
        entry["source"],
        f"{label}.source",
        message_def_id=message_def_id,
        schema_sha256=schema_sha256,
    )
    root = _parse_xml_bytes(schema_bytes, path)
    namespace, local = _split_xml_name(root.tag)
    if namespace != XML_SCHEMA_NS or local != "schema":
        raise FixtureManifestError(f"{path} root must be xs:schema")
    _require_schema_attributes(
        root,
        {"elementFormDefault", "targetNamespace"},
        path,
        "xs:schema root",
    )
    target_namespace = root.attrib.get("targetNamespace")
    if isinstance(target_namespace, str):
        _reject_overlong_xml_identifier(target_namespace, f"{path} targetNamespace")
        _reject_non_ascii_identifier(target_namespace, f"{path} targetNamespace")
        _reject_secret_looking_identifier(target_namespace, f"{path} targetNamespace")
    expected_namespace = _namespace_for(message_def_id)
    if target_namespace != expected_namespace:
        raise FixtureManifestError(
            f"{path} targetNamespace is {target_namespace!r}, expected {expected_namespace!r}"
        )
    if root.attrib.get("elementFormDefault") != "qualified":
        raise FixtureManifestError(f"{path} elementFormDefault must be qualified")
    _reject_unsupported_schema_composition(root, path)
    payload_root = _schema_payload_root(root, path)
    if payload_root != expected_payload_root:
        raise FixtureManifestError(
            f"{path} payload root is {payload_root!r}, expected {expected_payload_root!r}"
        )
    return {
        "path": rel_path,
        "message_def_id": message_def_id,
        "target_namespace": target_namespace,
        "payload_root": payload_root,
        "schema_only": schema_only_reason is not None,
        "schema_only_reason": schema_only_reason,
        "source": source,
        "sha256": schema_sha256,
    }


def _first_element_child(root: ET.Element, path: Path) -> ET.Element:
    children = [child for child in list(root) if isinstance(child.tag, str)]
    if len(children) != 1:
        raise FixtureManifestError(f"{path} Document must contain exactly one payload element")
    return children[0]


def _require_no_xml_attributes(element: ET.Element, path: Path, label: str) -> None:
    if element.attrib:
        raise FixtureManifestError(f"{path} {label} must not declare attributes")


def _validate_fixture_xml_schema(
    schema_path: Path,
    fixture_path: Path,
    label: str,
    xmllint_timeout_secs: float,
) -> None:
    xmllint = shutil.which("xmllint")
    if xmllint is None:
        raise FixtureManifestError(
            "--validate-xml-schema requires xmllint on PATH for offline XSD validation"
        )
    (
        returncode,
        stdout,
        stdout_truncated,
        stderr,
        stderr_truncated,
        timed_out,
    ) = _run_command_bounded(
        [
            xmllint,
            "--noout",
            "--nonet",
            "--schema",
            str(schema_path),
            str(fixture_path),
        ],
        MAX_XMLLINT_OUTPUT_BYTES,
        xmllint_timeout_secs,
    )
    if timed_out:
        raise FixtureManifestError(
            f"{label} xmllint timed out after {xmllint_timeout_secs:g} seconds"
        )
    output_truncated = stdout_truncated or stderr_truncated
    if returncode != 0:
        detail = _xmllint_output_detail(stderr or stdout)
        if detail:
            detail = ": " + detail
        if output_truncated:
            detail = (
                f"{detail} [xmllint output truncated at "
                f"{MAX_XMLLINT_OUTPUT_BYTES} bytes]"
            )
        raise FixtureManifestError(f"{label} failed XML schema validation{detail}")
    if output_truncated:
        raise FixtureManifestError(
            f"{label} xmllint output exceeded {MAX_XMLLINT_OUTPUT_BYTES} byte limit"
        )
    unexpected_output = _unexpected_xmllint_success_output(stdout, stderr, fixture_path)
    if unexpected_output is not None:
        detail = _xmllint_output_detail(unexpected_output)
        raise FixtureManifestError(
            f"{label} xmllint emitted unexpected output on successful XML schema "
            f"validation: {detail}"
        )


def _unexpected_xmllint_success_output(
    stdout: str,
    stderr: str,
    fixture_path: Path,
) -> str | None:
    allowed = {f"{fixture_path} validates"}
    for output in (stdout, stderr):
        stripped = output.strip()
        if stripped and stripped not in allowed:
            return stripped
    return None


def _xmllint_output_detail(output: str) -> str:
    detail = output.strip()
    if not detail:
        return ""
    if _contains_secret_material(detail) or _contains_secret_identifier_material(detail):
        return "[xmllint output redacted: secret-looking material]"
    if _contains_unsafe_diagnostic_control(detail):
        return "[xmllint output redacted: control characters]"
    if _contains_non_ascii_material(detail):
        return "[xmllint output redacted: non-ASCII material]"
    return detail[:4096]


def _contains_unsafe_diagnostic_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\t"}) or ord(ch) == 0x7F
        for ch in value
    )


def verify_fixture_entry(
    entry: dict[str, Any],
    label: str,
    manifest_dir: Path,
    schemas_by_path: dict[str, dict[str, Any]],
    *,
    validate_xml_schema: bool,
    xmllint_timeout_secs: float,
) -> dict[str, Any]:
    """Verify one XML fixture manifest entry and return normalized metadata."""

    _reject_unknown_keys(entry, FIXTURE_KEYS, label)
    rel_path = _required_string(entry, "path", label)
    message_def_id = _require_message_def_id(
        _required_string(entry, "message_def_id", label),
        f"{label}.message_def_id",
    )
    expected_payload_root = _required_string(entry, "payload_root", label)
    _reject_overlong_xml_identifier(expected_payload_root, f"{label}.payload_root")
    _reject_non_ascii_identifier(expected_payload_root, f"{label}.payload_root")
    _reject_secret_looking_identifier(expected_payload_root, f"{label}.payload_root")
    schema_rel = _optional_string(entry, "schema", label)
    missing_schema_reason = _optional_string(entry, "missing_schema_reason", label)
    if missing_schema_reason is not None:
        _reject_reviewed_gap_reason_material(
            missing_schema_reason,
            f"{label}.missing_schema_reason",
        )
    if schema_rel is not None and missing_schema_reason is not None:
        raise FixtureManifestError(f"{label} cannot set both schema and missing_schema_reason")
    if schema_rel is None and missing_schema_reason is None:
        raise FixtureManifestError(f"{label} must set schema or missing_schema_reason")
    if not rel_path.endswith(".xml"):
        raise FixtureManifestError(f"{label}.path must point to an .xml file")

    path = _validate_relative_path(
        rel_path,
        manifest_dir,
        manifest_dir.parent,
        f"{label}.path",
        allow_parent_segments=True,
    )
    fixture_bytes = _read_regular_file(path, max_bytes=MAX_FIXTURE_XML_BYTES)
    root = _parse_xml_bytes(fixture_bytes, path)
    _reject_non_ascii_xml_identifiers(root, path, "XML fixture")
    _reject_secret_looking_xml_content(root, path, "XML fixture")
    namespace, local = _split_xml_name(root.tag)
    if local != "Document":
        raise FixtureManifestError(f"{path} root element must be Document")
    namespace_message_id = _message_id_from_namespace(namespace, str(path))
    if namespace_message_id != message_def_id:
        raise FixtureManifestError(
            f"{path} namespace message id is {namespace_message_id!r}, expected {message_def_id!r}"
        )
    _require_no_xml_attributes(root, path, "Document element")
    payload = _first_element_child(root, path)
    payload_namespace, payload_local = _split_xml_name(payload.tag)
    if payload_namespace != namespace:
        raise FixtureManifestError(f"{path} payload namespace must match Document namespace")
    _reject_overlong_xml_identifier(payload_local, f"{path} payload root")
    _reject_non_ascii_identifier(payload_local, f"{path} payload root")
    _reject_secret_looking_identifier(payload_local, f"{path} payload root")
    if payload_local != expected_payload_root:
        raise FixtureManifestError(
            f"{path} payload root is {payload_local!r}, expected {expected_payload_root!r}"
        )
    _require_no_xml_attributes(payload, path, "payload element")

    schema_backed = False
    schema_validated = False
    if schema_rel is not None:
        if not schema_rel.endswith(".xsd"):
            raise FixtureManifestError(f"{label}.schema must point to an .xsd file")
        schema_path = _validate_relative_path(
            schema_rel,
            manifest_dir,
            manifest_dir,
            f"{label}.schema",
            allow_parent_segments=False,
        )
        schema = schemas_by_path.get(schema_rel)
        if schema is None:
            raise FixtureManifestError(f"{label}.schema references unknown schema {schema_rel}")
        if schema["message_def_id"] != message_def_id:
            raise FixtureManifestError(
                f"{label}.schema message id {schema['message_def_id']!r} "
                f"does not match fixture {message_def_id!r}"
            )
        if schema["payload_root"] != expected_payload_root:
            raise FixtureManifestError(
                f"{label}.schema payload root {schema['payload_root']!r} "
                f"does not match fixture {expected_payload_root!r}"
        )
        schema_backed = True
        if validate_xml_schema:
            _validate_fixture_xml_schema(
                schema_path,
                path,
                label,
                xmllint_timeout_secs,
            )
            schema_validated = True

    return {
        "path": rel_path,
        "message_def_id": message_def_id,
        "payload_root": payload_local,
        "schema": schema_rel,
        "schema_backed": schema_backed,
        "schema_validated": schema_validated,
        "missing_schema_reason": missing_schema_reason,
        "sha256": sha256_hex(fixture_bytes),
    }


def verify_profile_catalog(
    path: Path,
    schema_backed_message_ids: set[str],
) -> dict[str, Any]:
    """Verify profile catalog versions against schema-backed XML fixtures."""

    profiles, profile_catalog_sha256, profile_catalog_json_sha256 = _load_profile_catalog(path)
    versions: list[dict[str, Any]] = []
    missing_schema_versions: list[dict[str, str]] = []
    skipped_family_versions: list[dict[str, str]] = []
    seen_profile_ids: set[str] = set()
    seen_message_profiles: set[tuple[str, str, str]] = set()
    seen_profile_versions: set[tuple[str, str, str, str]] = set()

    for profile_offset, profile_raw in enumerate(profiles):
        profile_label = f"{path}.profiles[{profile_offset}]"
        profile = _require_object(profile_raw, profile_label)
        _reject_unknown_keys(profile, PROFILE_CATALOG_PROFILE_KEYS, profile_label)
        _check_no_secret_material(profile, profile_label)
        _validate_profile_catalog_profile_fields(profile, profile_label)
        profile_id = _required_string(profile, "id", profile_label)
        _reject_overlong_profile_catalog_identifier(profile_id, f"{profile_label}.id")
        if PROFILE_ID_RE.fullmatch(profile_id) is None:
            raise FixtureManifestError(
                f"{profile_label}.id must be a canonical lowercase profile id"
            )
        if profile_id in seen_profile_ids:
            raise FixtureManifestError(f"{profile_label}.id duplicates profile id {profile_id!r}")
        seen_profile_ids.add(profile_id)
        message_profiles = _require_array(
            profile.get("message_profiles"),
            f"{profile_label}.message_profiles",
        )
        if not message_profiles:
            raise FixtureManifestError(f"{profile_label}.message_profiles must not be empty")
        for message_offset, message_raw in enumerate(message_profiles):
            message_label = f"{profile_label}.message_profiles[{message_offset}]"
            message = _require_object(message_raw, message_label)
            _reject_unknown_keys(message, PROFILE_CATALOG_MESSAGE_KEYS, message_label)
            _validate_profile_catalog_message_fields(message, message_label)
            message_type = _required_string(message, "message_type", message_label)
            _reject_non_ascii_identifier(message_type, f"{message_label}.message_type")
            if MESSAGE_TYPE_RE.fullmatch(message_type) is None:
                raise FixtureManifestError(
                    f"{message_label}.message_type must be lowercase ISO family id"
                )
            direction = _required_string(message, "direction", message_label)
            if direction not in PROFILE_DIRECTIONS:
                raise FixtureManifestError(
                    f"{message_label}.direction must be one of "
                    + ", ".join(sorted(PROFILE_DIRECTIONS))
                )
            message_key = (profile_id, message_type, direction)
            if message_key in seen_message_profiles:
                raise FixtureManifestError(
                    f"{message_label} duplicates profile/message/direction entry"
                )
            seen_message_profiles.add(message_key)
            raw_versions = _require_array(
                message.get("versions"),
                f"{message_label}.versions",
            )
            if not raw_versions:
                raise FixtureManifestError(f"{message_label}.versions must not be empty")
            for version_offset, raw_version in enumerate(raw_versions):
                version_label = f"{message_label}.versions[{version_offset}]"
                if not isinstance(raw_version, str) or not raw_version.strip():
                    raise FixtureManifestError(
                        f"{version_label} must be a non-empty string"
                    )
                if raw_version != raw_version.strip():
                    raise FixtureManifestError(
                        f"{version_label} must not have surrounding whitespace"
                    )
                _reject_non_ascii_identifier(raw_version, version_label)
                _reject_secret_looking_identifier(raw_version, version_label)
                if MESSAGE_DEF_ID_RE.fullmatch(raw_version) is None:
                    if raw_version != message_type:
                        raise FixtureManifestError(
                            f"{version_label} must equal message_type {message_type!r} "
                            "or be a concrete message definition id"
                        )
                    key = (profile_id, message_type, direction, raw_version)
                    if key in seen_profile_versions:
                        raise FixtureManifestError(
                            f"{version_label} duplicates profile/message/direction "
                            f"family alias {raw_version!r}"
                        )
                    seen_profile_versions.add(key)
                    skipped_family_versions.append(
                        {
                            "profile_id": profile_id,
                            "message_type": message_type,
                            "direction": direction,
                            "version": raw_version,
                        }
                    )
                    continue
                if not raw_version.startswith(message_type + "."):
                    raise FixtureManifestError(
                        f"{version_label} {raw_version!r} does not match "
                        f"message_type {message_type!r}"
                    )
                key = (profile_id, message_type, direction, raw_version)
                if key in seen_profile_versions:
                    raise FixtureManifestError(
                        f"{version_label} duplicates profile/message/direction version "
                        f"{raw_version!r}"
                    )
                seen_profile_versions.add(key)
                schema_backed = raw_version in schema_backed_message_ids
                entry = {
                    "profile_id": profile_id,
                    "message_type": message_type,
                    "direction": direction,
                    "message_def_id": raw_version,
                    "schema_backed": schema_backed,
                }
                versions.append(entry)
                if not schema_backed:
                    missing_schema_versions.append(
                        {
                            "profile_id": profile_id,
                            "message_type": message_type,
                            "direction": direction,
                            "message_def_id": raw_version,
                        }
                    )

    return {
        "path": str(path),
        "sha256": profile_catalog_sha256,
        "catalog_json_sha256": profile_catalog_json_sha256,
        "profiles": len(profiles),
        "checked_versions": len(versions),
        "schema_backed_versions": sum(
            1 for version in versions if version["schema_backed"]
        ),
        "missing_schema_versions": missing_schema_versions,
        "skipped_family_versions": skipped_family_versions,
        "versions": versions,
    }


def verify_manifest(path: Path, args: argparse.Namespace) -> dict[str, Any]:
    """Verify the ISO fixture manifest and return a digest-bound summary."""

    xmllint_timeout_secs = _require_positive_finite_number(
        getattr(args, "xmllint_timeout_secs", DEFAULT_XMLLINT_TIMEOUT_SECS),
        "--xmllint-timeout-secs",
    )
    manifest_bytes = _read_regular_file(path, max_bytes=MAX_MANIFEST_JSON_BYTES)
    manifest = _require_object(_load_json_bytes(manifest_bytes, path), str(path))
    _reject_unknown_keys(manifest, TOP_LEVEL_KEYS, str(path))
    manifest_version = manifest.get("version")
    if (
        isinstance(manifest_version, bool)
        or not isinstance(manifest_version, int)
        or manifest_version != MANIFEST_VERSION
    ):
        raise FixtureManifestError(f"{path}.version must be {MANIFEST_VERSION}")
    manifest_dir = path.resolve().parent

    raw_schemas = _require_array(manifest.get("schemas"), f"{path}.schemas")
    raw_fixtures = _require_array(manifest.get("fixtures"), f"{path}.fixtures")
    if "blocked_schema_sources" not in manifest:
        raise FixtureManifestError(
            f"{path}.blocked_schema_sources must be recorded as an array"
        )
    raw_blocked_schema_sources = _require_array(
        manifest["blocked_schema_sources"],
        f"{path}.blocked_schema_sources",
    )
    schemas = [
        verify_schema_entry(
            _require_object(entry, f"{path}.schemas[{offset}]"),
            f"{path}.schemas[{offset}]",
            manifest_dir,
        )
        for offset, entry in enumerate(raw_schemas)
    ]
    schema_paths = [schema["path"] for schema in schemas]
    if len(schema_paths) != len(set(schema_paths)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate schema paths")
    schema_ids = [schema["message_def_id"] for schema in schemas]
    if len(schema_ids) != len(set(schema_ids)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate message_def_id values")
    schema_digests = [schema["sha256"] for schema in schemas]
    if len(schema_digests) != len(set(schema_digests)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate schema SHA-256 values")
    schema_sources = [
        (
            schema["source"]["repository"],
            schema["source"]["commit"],
            schema["source"]["path"],
        )
        for schema in schemas
    ]
    if len(schema_sources) != len(set(schema_sources)):
        raise FixtureManifestError(f"{path}.schemas contains duplicate source provenance")
    schemas_by_path = {schema["path"]: schema for schema in schemas}

    blocked_schema_sources = [
        _verify_blocked_schema_source(
            entry,
            f"{path}.blocked_schema_sources[{offset}]",
        )
        for offset, entry in enumerate(raw_blocked_schema_sources)
    ]
    blocked_source_provenance = [
        (
            blocked["source"]["repository"],
            blocked["source"]["commit"],
            blocked["source"]["path"],
        )
        for blocked in blocked_schema_sources
    ]
    if len(blocked_source_provenance) != len(set(blocked_source_provenance)):
        raise FixtureManifestError(
            f"{path}.blocked_schema_sources contains duplicate source provenance"
        )
    blocked_source_digests = [
        blocked["source"]["sha256"] for blocked in blocked_schema_sources
    ]
    if len(blocked_source_digests) != len(set(blocked_source_digests)):
        raise FixtureManifestError(
            f"{path}.blocked_schema_sources contains duplicate candidate SHA-256 values"
        )
    blocked_message_ids = {
        blocked["message_def_id"] for blocked in blocked_schema_sources
    }
    for message_def_id in sorted(blocked_message_ids & set(schema_ids)):
        raise FixtureManifestError(
            f"{path}.blocked_schema_sources includes already checked-in schema {message_def_id}"
        )

    fixtures = [
        verify_fixture_entry(
            _require_object(entry, f"{path}.fixtures[{offset}]"),
            f"{path}.fixtures[{offset}]",
            manifest_dir,
            schemas_by_path,
            validate_xml_schema=args.validate_xml_schema,
            xmllint_timeout_secs=xmllint_timeout_secs,
        )
        for offset, entry in enumerate(raw_fixtures)
    ]
    fixture_paths = [fixture["path"] for fixture in fixtures]
    if len(fixture_paths) != len(set(fixture_paths)):
        raise FixtureManifestError(f"{path}.fixtures contains duplicate fixture paths")
    fixture_digests = [fixture["sha256"] for fixture in fixtures]
    if len(fixture_digests) != len(set(fixture_digests)):
        raise FixtureManifestError(f"{path}.fixtures contains duplicate fixture SHA-256 values")
    backed_schema_paths = {fixture["schema"] for fixture in fixtures if fixture["schema"]}
    schema_only = [
        schema
        for schema in schemas
        if schema["path"] not in backed_schema_paths
    ]
    for schema in schema_only:
        if not schema["schema_only_reason"]:
            raise FixtureManifestError(
                f"{path} schema {schema['path']} has no fixture and no schema_only_reason"
            )
    missing_schema_fixtures = [
        fixture for fixture in fixtures if not fixture["schema_backed"]
    ]
    if args.require_schema_backed_fixtures and missing_schema_fixtures:
        first = missing_schema_fixtures[0]
        raise FixtureManifestError(
            f"{first['path']} is not schema-backed: {first['missing_schema_reason']}"
        )
    if args.require_fixture_for_schema and schema_only:
        first = schema_only[0]
        raise FixtureManifestError(
            f"{first['path']} has no standalone fixture: {first['schema_only_reason']}"
        )
    schema_backed_message_ids = {
        fixture["message_def_id"] for fixture in fixtures if fixture["schema_backed"]
    }
    profile_catalog_path = args.profile_catalog
    if profile_catalog_path is None and args.require_profile_schema_backed_versions:
        profile_catalog_path = DEFAULT_PROFILE_CATALOG
    profile_catalog = (
        verify_profile_catalog(profile_catalog_path, schema_backed_message_ids)
        if profile_catalog_path is not None
        else None
    )
    missing_profile_schema_versions = (
        profile_catalog["missing_schema_versions"] if profile_catalog else []
    )
    if profile_catalog is not None and blocked_message_ids:
        blocked_gap_message_ids = {
            fixture["message_def_id"] for fixture in missing_schema_fixtures
        } | {
            schema["message_def_id"] for schema in schema_only
        } | {
            missing["message_def_id"] for missing in missing_profile_schema_versions
        }
        for message_def_id in sorted(blocked_message_ids - blocked_gap_message_ids):
            raise FixtureManifestError(
                f"{path}.blocked_schema_sources includes {message_def_id} "
                "without a current missing schema/profile gap"
            )
    if args.require_profile_schema_backed_versions and missing_profile_schema_versions:
        first = missing_profile_schema_versions[0]
        raise FixtureManifestError(
            f"profile {first['profile_id']} version {first['message_def_id']} "
            "is not schema-backed by any checked-in XML fixture"
        )

    summary: dict[str, Any] = {
        "version": SUMMARY_VERSION,
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "manifest": str(path),
        "manifest_sha256": sha256_hex(manifest_bytes),
        "verified_schemas": len(schemas),
        "verified_fixtures": len(fixtures),
        "blocked_schema_source_count": len(blocked_schema_sources),
        "schema_backed_fixtures": len(fixtures) - len(missing_schema_fixtures),
        "schema_validated_fixtures": sum(
            1 for fixture in fixtures if fixture["schema_validated"]
        ),
        "profile_checked_versions": (
            profile_catalog["checked_versions"] if profile_catalog else 0
        ),
        "profile_schema_backed_versions": (
            profile_catalog["schema_backed_versions"] if profile_catalog else 0
        ),
        "missing_schema_fixtures": [
            {
                "path": fixture["path"],
                "message_def_id": fixture["message_def_id"],
                "reason": fixture["missing_schema_reason"],
            }
            for fixture in missing_schema_fixtures
        ],
        "schema_only_entries": [
            {
                "path": schema["path"],
                "message_def_id": schema["message_def_id"],
                "reason": schema["schema_only_reason"],
            }
            for schema in schema_only
        ],
        "missing_profile_schema_versions": missing_profile_schema_versions,
        "blocked_schema_sources": blocked_schema_sources,
        "schemas": schemas,
        "fixtures": fixtures,
        "profile_catalog": profile_catalog,
        "strict": {
            "require_schema_backed_fixtures": args.require_schema_backed_fixtures,
            "require_fixture_for_schema": args.require_fixture_for_schema,
            "require_profile_schema_backed_versions": (
                args.require_profile_schema_backed_versions
            ),
            "validate_xml_schema": args.validate_xml_schema,
        },
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(summary))
    return summary


def run(args: argparse.Namespace) -> int:
    summary = verify_manifest(args.manifest, args)
    text = json.dumps(summary, indent=2, sort_keys=True) + "\n"
    if args.summary_out is not None:
        _write_text_output(args.summary_out, text)
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 checked-in XSD/XML fixture manifest wiring.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--manifest",
        type=Path,
        default=DEFAULT_MANIFEST,
        help="Fixture manifest JSON to verify.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the verification summary JSON.",
    )
    parser.add_argument(
        "--require-schema-backed-fixtures",
        action="store_true",
        help="Fail if any manifest fixture lacks a checked-in XSD package.",
    )
    parser.add_argument(
        "--require-fixture-for-schema",
        action="store_true",
        help="Fail if any checked-in XSD lacks a standalone XML fixture.",
    )
    parser.add_argument(
        "--profile-catalog",
        type=Path,
        default=None,
        help=(
            "Optional Rust profile catalog file containing DEFAULT_PROFILES_JSON "
            f"(default catalog: {DEFAULT_PROFILE_CATALOG})."
        ),
    )
    parser.add_argument(
        "--require-profile-schema-backed-versions",
        action="store_true",
        help=(
            "Fail if any concrete message version advertised by --profile-catalog "
            "lacks a schema-backed checked-in XML fixture; when --profile-catalog "
            "is omitted this uses the default catalog."
        ),
    )
    parser.add_argument(
        "--validate-xml-schema",
        action="store_true",
        help="Validate every schema-backed XML fixture against its checked-in XSD with xmllint.",
    )
    parser.add_argument(
        "--xmllint-timeout-secs",
        type=float,
        default=DEFAULT_XMLLINT_TIMEOUT_SECS,
        help="Maximum wall-clock seconds allowed for each xmllint validation.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        _preflight_raw_cli_secrets(
            argv,
            {
                "--manifest",
                "--profile-catalog",
                "--summary-out",
                "--xmllint-timeout-secs",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--require-fixture-for-schema",
                "--require-profile-schema-backed-versions",
                "--require-schema-backed-fixtures",
                "--validate-xml-schema",
            },
        )
        _preflight_numeric_cli_values(
            argv,
            integer_flags=set(),
            number_flags={"--xmllint-timeout-secs"},
        )
        _preflight_output_cli_paths(
            argv,
            {"--manifest", "--profile-catalog", "--summary-out"},
        )
        args = parser.parse_args(argv)
        return run(args)
    except FixtureManifestError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
