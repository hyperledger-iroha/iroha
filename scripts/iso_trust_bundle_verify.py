#!/usr/bin/env python3
"""Verify ISO 20022 XMLDSig/XAdES operator trust bundles.

Purpose:
  This operator-side preflight validates JSON trust bundles before their
  profile trust material is merged into Torii ISO bridge configuration. It
  checks canonical SHA-256 pins, digest-bound DER blobs, duplicate material,
  revocation-material requirements, HTTPS provenance, and absence of
  secret-looking fields or values.

Prerequisites:
  Python 3.11+. No third party Python packages are required.

Safety:
  The script is read-only unless ``--emit-profile-json`` or ``--summary-out`` is
  supplied. It does not contact remote endpoints. Runtime secrets such as bearer
  tokens, private keys, and authorization headers are rejected if they appear in
  the bundle.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import errno
import hashlib
import ipaddress
import json
import os
import re
import secrets
import stat
import sys
import unicodedata
import urllib.parse
from dataclasses import dataclass
from pathlib import Path
from typing import Any


BUNDLE_VERSION = 1
TRUST_SUMMARY_VERSION = 1
MAX_DER_BLOBS = 8
MAX_DER_BYTES = 1024 * 1024
MAX_DER_BASE64_CHARS = ((MAX_DER_BYTES + 2) // 3) * 4
MAX_BUNDLE_JSON_BYTES = 64 * 1024 * 1024
MAX_BUNDLE_INPUT_PATHS = 64
MAX_JSON_LIST_ITEMS = 8192
MAX_JSON_OBJECT_MEMBERS = 8192
MAX_JSON_NESTING_DEPTH = 128
MAX_SOURCE_URL_CHARS = 2048
MAX_LOCAL_PATH_CHARS = 4096
MAX_CLEAN_STRING_CHARS = 4096
REPOSITORY_XML_FIXTURE_PARTS = (
    "fixtures",
    "iso20022",
)
MAX_PROFILE_ID_CHARS = 128
MAX_TRUST_POLICY_CHARS = 128
MAX_TRUST_SOURCE_TEXT_CHARS = 256
MAX_TIMESTAMP_CHARS = 128
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
LOCAL_REBINDING_HOST_SUFFIXES = {"localtest.me", "lvh.me", "nip.io", "sslip.io", "vcap.me"}
NAT64_WELL_KNOWN_PREFIX = ipaddress.ip_network("64:ff9b::/96")
IPV4_COMPATIBLE_IPV6_PREFIX = ipaddress.ip_network("::/96")
POLICIES = {"record-only", "reject-unsupported", "require-verified"}
REQUIRE_VERIFIED = "require-verified"
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
KNOWN_RAILS = {
    "generic-iso20022",
    "swift-cbpr-plus",
    "fedwire-funds",
    "sepa-sct-inst",
    "securities-csd",
}
TOP_LEVEL_KEYS = {
    "version",
    "profile_id",
    "rail",
    "environment",
    "source",
    "embedded_signature_policy",
    "signature_public_key_sha256_pins",
    "trusted_public_key_sha256",
    "x509_trust_anchor_sha256_pins",
    "trusted_certificate_sha256",
    "x509_trust_anchors",
    "revoked_certificate_sha256",
    "revoked_certificates",
    "x509_required_certificate_policy_oids",
    "x509_require_crl_revocation_check",
    "x509_crls",
    "x509_require_ocsp_revocation_check",
    "x509_ocsp_responses",
}
SOURCE_KEYS = {"authority", "retrieved_at", "url", "version"}
DER_OBJECT_KEYS = {"label", "der_base64", "sha256"}
DER_KIND_CERTIFICATE = "X.509 certificate"
DER_KIND_CRL = "X.509 CRL"
DER_KIND_OCSP = "OCSP response"
OID_OCSP_BASIC_RESPONSE_DER = b"\x2b\x06\x01\x05\x05\x07\x30\x01\x01"
SUMMARY_DIGEST_FIELD = "summary_sha256"


class TrustBundleError(RuntimeError):
    """Raised when an ISO trust bundle is malformed or unsafe."""


@dataclass(frozen=True)
class DerElement:
    """One parsed DER TLV element."""

    tag: int
    header_len: int
    length: int
    start: int
    value_start: int
    end: int
    value: bytes


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
        raise TrustBundleError("max file bytes must be a positive integer")
    _reject_symlinked_existing_ancestors(path.parent, display_label=label)
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise TrustBundleError(f"{label} does not exist") from error
    mode = metadata.st_mode
    if stat.S_ISLNK(mode):
        raise TrustBundleError(f"{label} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise TrustBundleError(f"{label} must be a regular file")
    if max_bytes is not None and metadata.st_size > max_bytes:
        raise TrustBundleError(f"{label} exceeds {max_bytes} byte JSON limit")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        fd_metadata = os.fstat(fd)
        if not stat.S_ISREG(fd_metadata.st_mode):
            raise TrustBundleError(f"{label} must be a regular file")
        if max_bytes is not None and fd_metadata.st_size > max_bytes:
            raise TrustBundleError(f"{label} exceeds {max_bytes} byte JSON limit")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            limit = max_bytes + 1 if max_bytes is not None else -1
            raw = handle.read(limit)
        if max_bytes is not None and len(raw) > max_bytes:
            raise TrustBundleError(f"{label} exceeds {max_bytes} byte JSON limit")
        return raw
    except FileNotFoundError as error:
        raise TrustBundleError(f"{label} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise TrustBundleError(f"{label} must not be a symlink") from error
        raise TrustBundleError(f"cannot open {label} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _reject_output_path_smuggling(path: Path, label: str) -> None:
    raw = str(path)
    if not raw or not path.name:
        raise TrustBundleError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise TrustBundleError(
            f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters"
        )
    if _has_unsafe_control(raw):
        raise TrustBundleError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise TrustBundleError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise TrustBundleError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise TrustBundleError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise TrustBundleError(f"{label} must use forward slashes")
    if ";" in raw:
        raise TrustBundleError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise TrustBundleError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise TrustBundleError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = path.parts[1:] if path.is_absolute() else path.parts
    if any(part.startswith("-") for part in parts if part):
        raise TrustBundleError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in parts):
        raise TrustBundleError(f"{label} must not contain dot or parent segments")


def _reject_raw_output_path_smuggling(raw: str, label: str) -> None:
    if not raw:
        raise TrustBundleError(f"{label} must be a non-empty path")
    if len(raw) > MAX_LOCAL_PATH_CHARS:
        raise TrustBundleError(
            f"{label} must be no longer than {MAX_LOCAL_PATH_CHARS} characters"
        )
    if _has_unsafe_control(raw):
        raise TrustBundleError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise TrustBundleError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise TrustBundleError(f"{label} must not contain whitespace")
    if raw.startswith("-"):
        raise TrustBundleError(f"{label} must not start with a dash")
    if "\\" in raw:
        raise TrustBundleError(f"{label} must use forward slashes")
    if ";" in raw:
        raise TrustBundleError(f"{label} must not contain semicolon path parameters")
    if ":" in raw:
        raise TrustBundleError(f"{label} must not contain URI or drive prefixes")
    if _contains_secret_material(raw) or _contains_secret_identifier_material(raw):
        raise TrustBundleError(f"{label} must not contain secret-looking material")
    _reject_percent_encoded_path_smuggling(raw, label)
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise TrustBundleError(f"{label} must not contain empty path segments")
    if any(part.startswith("-") for part in checked_parts):
        raise TrustBundleError(f"{label} must not contain leading-dash path segments")
    if any(part in {".", ".."} for part in checked_parts):
        raise TrustBundleError(f"{label} must not contain dot or parent segments")


def _preflight_raw_cli_secrets(argv: list[str] | None, value_flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise TrustBundleError("argument terminator is not supported")
        if arg in value_flags:
            index += 2
            continue
        if any(arg.startswith(f"{flag}=") for flag in value_flags):
            index += 1
            continue
        if _has_unsafe_control(arg):
            raise TrustBundleError("CLI argument must not contain control characters")
        if any(ord(ch) > 0x7E for ch in arg):
            raise TrustBundleError("CLI argument must use printable ASCII")
        if _contains_secret_material(arg) or _contains_secret_identifier_material(arg):
            raise TrustBundleError("CLI argument must not contain secret-looking material")
        index += 1


def _preflight_boolean_cli_flags(argv: list[str] | None, flags: set[str]) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise TrustBundleError("argument terminator is not supported")
        flag, separator, _value = arg.partition("=")
        if separator and flag in flags:
            raise TrustBundleError(f"{flag} does not take a value")
        if (
            arg in flags
            and index + 1 < len(raw_args)
            and not raw_args[index + 1].startswith("--")
        ):
            raise TrustBundleError(f"{arg} does not take a value")
        index += 1


def _reject_raw_positive_int_cli_value(raw: str, flag: str) -> None:
    if raw != raw.strip() or _has_unsafe_control(raw):
        raise TrustBundleError(f"{flag} must be a positive integer")
    if any(ord(ch) > 0x7E for ch in raw):
        raise TrustBundleError(f"{flag} must use printable ASCII")
    try:
        value = int(raw, 10)
    except ValueError as error:
        raise TrustBundleError(f"{flag} must be a positive integer") from error
    if value <= 0:
        raise TrustBundleError(f"{flag} must be a positive integer")


def _preflight_positive_int_cli_values(
    argv: list[str] | None,
    flags: set[str],
) -> None:
    raw_args = sys.argv[1:] if argv is None else argv
    index = 0
    while index < len(raw_args):
        arg = raw_args[index]
        if arg == "--":
            raise TrustBundleError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise TrustBundleError(f"{flag} requires a positive integer value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise TrustBundleError(f"{flag} requires a positive integer value")
                _reject_raw_positive_int_cli_value(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise TrustBundleError(f"{flag} requires a positive integer value")
                _reject_raw_positive_int_cli_value(value, flag)
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
            raise TrustBundleError("argument terminator is not supported")
        matched = False
        for flag in flags:
            if arg == flag:
                if index + 1 >= len(raw_args):
                    raise TrustBundleError(f"{flag} requires a path value")
                value = raw_args[index + 1]
                if not value or value.startswith("--"):
                    raise TrustBundleError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 2
                matched = True
                break
            prefix = f"{flag}="
            if arg.startswith(prefix):
                value = arg[len(prefix) :]
                if not value or value.startswith("--"):
                    raise TrustBundleError(f"{flag} requires a path value")
                _reject_raw_output_path_smuggling(value, flag)
                index += 1
                matched = True
                break
        if not matched:
            index += 1


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


def _output_path_is_repository_iso_fixture(raw: str) -> bool:
    return _path_contains_component_sequence(raw, REPOSITORY_XML_FIXTURE_PARTS)


def _reject_repository_output_path(path: Path, label: str) -> None:
    if _output_path_is_repository_iso_fixture(str(path)):
        raise TrustBundleError(
            f"{label} must not point to checked-in ISO fixture artifacts"
        )


def _same_existing_file(left: Path, right: Path) -> bool:
    try:
        left_stat = left.stat()
        right_stat = right.stat()
    except FileNotFoundError:
        return False
    except OSError:
        return False
    return os.path.samestat(left_stat, right_stat)


def _reject_output_input_alias(
    output_path: Path | None,
    output_label: str,
    inputs: tuple[tuple[str, Path], ...],
) -> None:
    if output_path is None:
        return
    for input_label, input_path in inputs:
        if str(output_path) == str(input_path) or _same_existing_file(
            output_path,
            input_path,
        ):
            raise TrustBundleError(
                f"{output_label} must not reuse {input_label} path"
            )


def _reject_output_output_alias(
    left: Path | None,
    left_label: str,
    right: Path | None,
    right_label: str,
) -> None:
    if left is None or right is None:
        return
    if str(left) == str(right) or _same_existing_file(left, right):
        raise TrustBundleError(f"{left_label} and {right_label} must be different paths")


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
        raise TrustBundleError(f"{label} must be a directory") from error
    if create_parent:
        try:
            path.parent.mkdir(parents=True, exist_ok=True)
        except FileExistsError as error:
            raise TrustBundleError(f"{label} must be a directory") from error
    if path.parent.exists() or path.parent.is_symlink():
        parent_mode = path.parent.lstat().st_mode
        if stat.S_ISLNK(parent_mode):
            raise TrustBundleError(f"{label} must not be a symlink")
        if not stat.S_ISDIR(parent_mode):
            raise TrustBundleError(f"{label} must be a directory")
    if path.exists() or path.is_symlink():
        metadata = path.lstat()
        if stat.S_ISLNK(metadata.st_mode):
            raise TrustBundleError(f"{label} must not be a symlink")
        if not stat.S_ISREG(metadata.st_mode):
            raise TrustBundleError(f"{label} must be a regular file")
        if metadata.st_nlink > 1:
            raise TrustBundleError(f"{label} must not be hard-linked")


def _write_text_output(path: Path, text: str, *, display_label: str | None = None) -> None:
    label = display_label if display_label is not None else "output path"
    _ensure_text_output_target(path, display_label=label)
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise TrustBundleError(f"{label} must not be a symlink") from error
        raise TrustBundleError(f"{label} must be a directory") from error

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
                raise TrustBundleError(f"{label} temp file must not be a symlink") from error
            raise TrustBundleError(
                f"cannot open temporary output for {label}: {error.strerror}"
            ) from error
        opened = os.fstat(fd)
        if not stat.S_ISREG(opened.st_mode):
            raise TrustBundleError(f"{label} temp file must be a regular file")
        if opened.st_nlink > 1:
            raise TrustBundleError(f"{label} temp file must not be hard-linked")
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
        if stat.S_ISLNK(mode):
            if path.is_absolute() and current.parent == Path(path.anchor):
                continue
            label = display_label or str(current)
            raise TrustBundleError(f"{label} must not be a symlink")


def _load_json(path: Path, *, display_label: str | None = None) -> Any:
    label = display_label or str(path)
    try:
        raw = _read_regular_file(
            path,
            max_bytes=MAX_BUNDLE_JSON_BYTES,
            display_label=label,
        )
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise TrustBundleError(f"{label} is not UTF-8 JSON") from error
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
            parse_constant=_reject_json_constant,
        )
    except json.JSONDecodeError as error:
        raise TrustBundleError(f"{label} is not valid JSON: {error}") from error
    except RecursionError as error:
        raise TrustBundleError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        ) from error
    _reject_json_surrogates(value)
    return value


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    if len(pairs) > MAX_JSON_OBJECT_MEMBERS:
        raise TrustBundleError(
            f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
        )
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise TrustBundleError("JSON object contains duplicate key")
        seen.add(key)
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    raise TrustBundleError("JSON contains non-finite numeric constant")


def _reject_json_surrogates(value: Any, *, _depth: int = 0) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise TrustBundleError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, str):
        if any(0xD800 <= ord(ch) <= 0xDFFF for ch in value):
            raise TrustBundleError("JSON contains invalid Unicode surrogate")
    elif isinstance(value, list):
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise TrustBundleError(
                f"JSON array must contain at most {MAX_JSON_LIST_ITEMS} items"
            )
        for item in value:
            _reject_json_surrogates(item, _depth=_depth + 1)
    elif isinstance(value, dict):
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise TrustBundleError(
                f"JSON object must contain at most {MAX_JSON_OBJECT_MEMBERS} members"
            )
        for key, item in value.items():
            _reject_json_surrogates(key, _depth=_depth + 1)
            _reject_json_surrogates(item, _depth=_depth + 1)


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise TrustBundleError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    if set(value) - allowed:
        raise TrustBundleError(f"{label} contains unknown keys")


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
    return _has_unsafe_control(str(value))


def _contains_unsafe_json_control(value: str) -> bool:
    return any(
        (ord(ch) < 0x20 and ch not in {"\n", "\r", "\t"})
        or ord(ch) == 0x7F
        or unicodedata.category(ch) == "Cf"
        for ch in value
    )


def _reject_secret_looking_identifier(value: str, label: str) -> None:
    if _contains_secret_material(value) or _is_secret_looking_key(value):
        raise TrustBundleError(f"{label} must not contain secret-looking material")


def _reject_non_ascii_context(value: str, label: str) -> None:
    if any(ord(ch) > 0x7E for ch in value):
        raise TrustBundleError(f"{label} must use printable ASCII")


def _reject_overlong_trust_policy(value: str, label: str) -> None:
    if len(value) > MAX_TRUST_POLICY_CHARS:
        raise TrustBundleError(
            f"{label} must be no longer than {MAX_TRUST_POLICY_CHARS} characters"
        )


def _reject_overlong_trust_source_text(value: str, label: str) -> None:
    if len(value) > MAX_TRUST_SOURCE_TEXT_CHARS:
        raise TrustBundleError(
            f"{label} must be no longer than {MAX_TRUST_SOURCE_TEXT_CHARS} characters"
        )


def _required_context_string(bundle: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(bundle, key, label)
    _reject_non_ascii_context(raw, f"{label}.{key}")
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    return raw


def _check_no_secret_material(value: Any, path: str = "$", *, _depth: int = 0) -> None:
    if _depth > MAX_JSON_NESTING_DEPTH:
        raise TrustBundleError(
            f"JSON nesting depth must be at most {MAX_JSON_NESTING_DEPTH} levels"
        )
    if isinstance(value, dict):
        if len(value) > MAX_JSON_OBJECT_MEMBERS:
            raise TrustBundleError(
                f"{path} must contain at most {MAX_JSON_OBJECT_MEMBERS} object members"
            )
        for key, child in value.items():
            if _is_secret_looking_key(key):
                raise TrustBundleError(f"{path} contains forbidden secret-looking field")
            if _is_control_bearing_key(key):
                raise TrustBundleError(f"{path} contains forbidden control-bearing field")
            _check_no_secret_material(child, f"{path}.{key}", _depth=_depth + 1)
    elif isinstance(value, list):
        if len(value) > MAX_JSON_LIST_ITEMS:
            raise TrustBundleError(
                f"{path} must contain at most {MAX_JSON_LIST_ITEMS} items"
            )
        for offset, child in enumerate(value):
            _check_no_secret_material(child, f"{path}[{offset}]", _depth=_depth + 1)
    elif isinstance(value, str):
        if _contains_unsafe_json_control(value):
            raise TrustBundleError(f"{path} contains unsafe control characters")
        if _contains_secret_material(value):
            raise TrustBundleError(f"{path} contains secret-looking material")


def _has_unsafe_control(value: str) -> bool:
    return any(
        ord(char) < 0x20 or ord(char) == 0x7F or unicodedata.category(char) == "Cf"
        for char in value
    )


def _reject_percent_encoded_path_smuggling(raw: str, label: str) -> None:
    index = 0
    while True:
        index = raw.find("%", index)
        if index == -1:
            return
        token = raw[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise TrustBundleError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise TrustBundleError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        if byte in {0x2E, 0x2F, 0x5C}:
            raise TrustBundleError(
                f"{label} must not contain encoded dot or separator characters"
            )
        if byte == 0x3B:
            raise TrustBundleError(
                f"{label} must not contain encoded semicolon parameters"
            )
        if byte in {0x23, 0x3A, 0x3F, 0x40, 0x5B, 0x5D}:
            raise TrustBundleError(
                f"{label} must not contain encoded URL delimiter characters"
            )
        if byte == 0x25:
            raise TrustBundleError(f"{label} must not contain encoded percent characters")
        index += 3


def _reject_unsafe_control(value: str, label: str) -> None:
    if _has_unsafe_control(value):
        raise TrustBundleError(f"{label} must not contain control characters")


def _reject_url_percent_encoding_smuggling(url: str, label: str) -> None:
    index = 0
    while True:
        index = url.find("%", index)
        if index == -1:
            return
        token = url[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise TrustBundleError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise TrustBundleError(
                f"{label} must not contain percent-encoded control or space characters"
            )
        index += 3


def _required_string(
    bundle: dict[str, Any],
    key: str,
    label: str,
    *,
    max_chars: int | None = MAX_CLEAN_STRING_CHARS,
) -> str:
    raw = bundle.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise TrustBundleError(f"{label}.{key} must be a non-empty string")
    if max_chars is not None and len(raw) > max_chars:
        raise TrustBundleError(f"{label}.{key} must be no longer than {max_chars} characters")
    _reject_unsafe_control(raw, f"{label}.{key}")
    if raw != raw.strip():
        raise TrustBundleError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _required_profile_id(bundle: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(bundle, key, label)
    if len(raw) > MAX_PROFILE_ID_CHARS:
        raise TrustBundleError(
            f"{label}.{key} must be no longer than {MAX_PROFILE_ID_CHARS} characters"
        )
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    if PROFILE_ID_RE.fullmatch(raw) is None:
        raise TrustBundleError(f"{label}.{key} must be a canonical lowercase profile id")
    return raw


def _required_rail(bundle: dict[str, Any], key: str, label: str) -> str:
    raw = _required_string(bundle, key, label)
    _reject_secret_looking_identifier(raw, f"{label}.{key}")
    if raw not in KNOWN_RAILS:
        raise TrustBundleError(
            f"{label}.{key} must be one of " + ", ".join(sorted(KNOWN_RAILS))
        )
    return raw


def _optional_string(
    bundle: dict[str, Any],
    key: str,
    label: str,
    *,
    max_chars: int | None = MAX_CLEAN_STRING_CHARS,
) -> str | None:
    if key not in bundle:
        return None
    raw = bundle.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise TrustBundleError(f"{label}.{key} must be a non-empty string when provided")
    if max_chars is not None and len(raw) > max_chars:
        raise TrustBundleError(f"{label}.{key} must be no longer than {max_chars} characters")
    _reject_unsafe_control(raw, f"{label}.{key}")
    if raw != raw.strip():
        raise TrustBundleError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _optional_positive_cli_int(value: Any, label: str) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool):
        raise TrustBundleError(f"{label} must be a positive integer")
    if isinstance(value, int):
        parsed = value
    elif isinstance(value, str):
        if any(ord(ch) > 0x7E for ch in value):
            raise TrustBundleError(f"{label} must use printable ASCII")
        if value != value.strip() or not value.isdecimal():
            raise TrustBundleError(f"{label} must be a positive integer")
        parsed = int(value)
    else:
        raise TrustBundleError(f"{label} must be a positive integer")
    if parsed <= 0:
        raise TrustBundleError(f"{label} must be a positive integer")
    return parsed


def _required_cli_bool(value: Any, label: str) -> bool:
    if not isinstance(value, bool):
        raise TrustBundleError(f"{label} must be a boolean")
    return value


def _optional_cli_path(value: Any, label: str) -> Path | None:
    if value is None:
        return None
    if isinstance(value, bytes):
        raise TrustBundleError(f"{label} must be a path")
    try:
        return Path(value)
    except TypeError as error:
        raise TrustBundleError(f"{label} must be a path") from error


def _required_cli_path_sequence(value: Any, label: str) -> list[Path]:
    if value is None:
        return []
    if isinstance(value, (str, bytes)) or not isinstance(value, (list, tuple)):
        raise TrustBundleError(f"{label} must be a repeatable path list")
    if len(value) > MAX_BUNDLE_INPUT_PATHS:
        raise TrustBundleError(f"{label} accepts at most {MAX_BUNDLE_INPUT_PATHS} paths")
    paths: list[Path] = []
    for offset, entry in enumerate(value):
        if isinstance(entry, bytes):
            raise TrustBundleError(f"{label}[{offset}] must be a path")
        try:
            paths.append(Path(entry))
        except TypeError as error:
            raise TrustBundleError(f"{label}[{offset}] must be a path") from error
    return paths


def _require_policy_booleans(args: argparse.Namespace) -> None:
    for attr, label in (
        ("allow_record_only", "--allow-record-only"),
        ("allow_insecure_source_url", "--allow-insecure-source-url"),
        ("allow_synthetic_der", "--allow-synthetic-der"),
    ):
        setattr(args, attr, _required_cli_bool(getattr(args, attr, None), label))


def _required_bool(bundle: dict[str, Any], key: str, label: str) -> bool:
    raw = bundle.get(key)
    if not isinstance(raw, bool):
        raise TrustBundleError(f"{label}.{key} must be a boolean")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _validate_sha256(value: str, label: str) -> str:
    _reject_secret_looking_identifier(value, label)
    if not _is_lower_sha256(value):
        raise TrustBundleError(f"{label} must be canonical lowercase SHA-256 hex")
    if all(ch == "0" for ch in value):
        raise TrustBundleError(f"{label} must not be all zero")
    return value


def _required_list_field(
    bundle: dict[str, Any],
    key: str,
    label: str,
    description: str,
) -> Any:
    if key not in bundle:
        raise TrustBundleError(f"{label}.{key} must be recorded as an array of {description}")
    value = bundle[key]
    if not isinstance(value, list):
        return value
    if len(value) > MAX_JSON_LIST_ITEMS:
        raise TrustBundleError(
            f"{label}.{key} must contain at most {MAX_JSON_LIST_ITEMS} items"
        )
    return value


def _sha256_list(bundle: dict[str, Any], key: str, label: str) -> list[str]:
    raw = _required_list_field(bundle, key, label, "SHA-256 strings")
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of SHA-256 strings")
    result: list[str] = []
    seen: set[str] = set()
    for offset, item in enumerate(raw):
        if not isinstance(item, str):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a SHA-256 string")
        if item != item.strip():
            raise TrustBundleError(f"{label}.{key}[{offset}] must not have surrounding whitespace")
        digest = _validate_sha256(item, f"{label}.{key}[{offset}]")
        if digest in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates SHA-256")
        seen.add(digest)
        result.append(digest)
    return sorted(result)


def _oid_list(bundle: dict[str, Any], key: str, label: str) -> list[str]:
    raw = _required_list_field(bundle, key, label, "dotted numeric OIDs")
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of dotted numeric OIDs")
    result: list[str] = []
    seen: set[str] = set()
    for offset, item in enumerate(raw):
        if not isinstance(item, str):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
        if len(item) > MAX_CLEAN_STRING_CHARS:
            raise TrustBundleError(
                f"{label}.{key}[{offset}] must be no longer than {MAX_CLEAN_STRING_CHARS} characters"
            )
        if item != item.strip():
            raise TrustBundleError(f"{label}.{key}[{offset}] must not have surrounding whitespace")
        value = item
        _reject_secret_looking_identifier(value, f"{label}.{key}[{offset}]")
        if not _valid_oid(value):
            raise TrustBundleError(f"{label}.{key}[{offset}] must be a dotted numeric OID")
        if value in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates OID")
        seen.add(value)
        result.append(value)
    return sorted(result)


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


def _strict_base64_der(
    value: str,
    label: str,
    *,
    kind: str,
    allow_synthetic_der: bool,
) -> tuple[bytes, str, bool]:
    raw = value.strip()
    if len(raw) > MAX_DER_BASE64_CHARS:
        raise TrustBundleError(
            f"{label} must decode to no more than {MAX_DER_BYTES} bytes"
        )
    try:
        der = base64.b64decode(raw, validate=True)
    except ValueError as error:
        raise TrustBundleError(f"{label} must be canonical base64 DER") from error
    if not der or len(der) > MAX_DER_BYTES:
        raise TrustBundleError(f"{label} must be non-empty DER no larger than {MAX_DER_BYTES} bytes")
    _require_der_sequence(der, label)
    matches_kind = _der_matches_kind(der, label, kind)
    if not allow_synthetic_der and not matches_kind:
        _raise_der_kind_error(label, kind)
    canonical = base64.b64encode(der).decode("ascii")
    if canonical != raw:
        raise TrustBundleError(f"{label} must be canonical padded base64")
    return der, canonical, not matches_kind


def _require_der_sequence(der: bytes, label: str) -> None:
    root = _read_der_element(der, 0, label)
    if root.tag != 0x30:
        raise TrustBundleError(f"{label} must be a DER SEQUENCE")
    if root.end != len(der):
        raise TrustBundleError(f"{label} DER length does not consume the whole value")


def _read_der_element(data: bytes, offset: int, label: str) -> DerElement:
    if offset >= len(data):
        raise TrustBundleError(f"{label} has truncated DER")
    if len(data) - offset < 2:
        raise TrustBundleError(f"{label} has truncated DER length")
    tag = data[offset]
    length_byte = data[offset + 1]
    if length_byte < 0x80:
        length = length_byte
        header_len = 2
    else:
        length_len = length_byte & 0x7F
        if length_len == 0:
            raise TrustBundleError(f"{label} must not use BER indefinite length")
        if length_len > 4 or len(data) - offset < 2 + length_len:
            raise TrustBundleError(f"{label} has invalid DER length")
        length_bytes = data[offset + 2 : offset + 2 + length_len]
        if length_bytes[0] == 0:
            raise TrustBundleError(f"{label} has non-minimal DER length")
        length = int.from_bytes(length_bytes, "big")
        if length < 0x80:
            raise TrustBundleError(f"{label} must use short DER length form")
        header_len = 2 + length_len
    end = offset + header_len + length
    if end > len(data):
        raise TrustBundleError(f"{label} has truncated DER value")
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


def _root_children(der: bytes, label: str) -> list[DerElement]:
    root = _read_der_element(der, 0, label)
    if root.tag != 0x30:
        raise TrustBundleError(f"{label} must be a DER SEQUENCE")
    if root.end != len(der):
        raise TrustBundleError(f"{label} DER length does not consume the whole value")
    return _der_children(root, label)


def _require_der_kind(der: bytes, label: str, kind: str) -> None:
    if not _der_matches_kind(der, label, kind):
        _raise_der_kind_error(label, kind)


def _der_matches_kind(der: bytes, label: str, kind: str) -> bool:
    if kind == DER_KIND_CERTIFICATE:
        return _looks_like_x509_certificate(der, label)
    elif kind == DER_KIND_CRL:
        return _looks_like_x509_crl(der, label)
    elif kind == DER_KIND_OCSP:
        return _looks_like_ocsp_response(der, label)
    else:  # pragma: no cover - internal caller bug.
        raise TrustBundleError(f"{label} has unsupported DER kind")


def _raise_der_kind_error(label: str, kind: str) -> None:
    if kind == DER_KIND_CERTIFICATE:
        raise TrustBundleError(f"{label} must look like an X.509 certificate")
    if kind == DER_KIND_CRL:
        raise TrustBundleError(f"{label} must look like an X.509 CRL")
    if kind == DER_KIND_OCSP:
        raise TrustBundleError(f"{label} must look like an OCSPResponse")
    raise TrustBundleError(f"{label} has unsupported DER kind")


def _looks_like_algorithm_identifier(element: DerElement, label: str) -> bool:
    if element.tag != 0x30:
        return False
    children = _der_children(element, label)
    return bool(children) and children[0].tag == 0x06


def _looks_like_x509_certificate(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
    if len(children) != 3 or children[0].tag != 0x30 or children[2].tag != 0x03:
        return False
    if not _looks_like_algorithm_identifier(children[1], label):
        return False
    tbs_children = _der_children(children[0], label)
    cursor = 1 if tbs_children and tbs_children[0].tag == 0xA0 else 0
    if len(tbs_children) < cursor + 6:
        return False
    return (
        tbs_children[cursor].tag == 0x02
        and _looks_like_algorithm_identifier(tbs_children[cursor + 1], label)
        and tbs_children[cursor + 2].tag == 0x30
        and tbs_children[cursor + 3].tag == 0x30
        and tbs_children[cursor + 4].tag == 0x30
        and tbs_children[cursor + 5].tag == 0x30
    )


def _looks_like_x509_crl(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
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


def _looks_like_ocsp_response(der: bytes, label: str) -> bool:
    children = _root_children(der, label)
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


def _der_objects(
    bundle: dict[str, Any],
    key: str,
    label: str,
    *,
    kind: str,
    allow_synthetic_der: bool,
) -> tuple[list[dict[str, Any]], list[str], bool]:
    raw = _required_list_field(bundle, key, label, "DER objects")
    if not isinstance(raw, list):
        raise TrustBundleError(f"{label}.{key} must be an array of DER objects")
    if len(raw) > MAX_DER_BLOBS:
        raise TrustBundleError(f"{label}.{key} must not contain more than {MAX_DER_BLOBS} entries")
    entries: list[dict[str, Any]] = []
    seen: set[str] = set()
    seen_labels: set[str] = set()
    uses_synthetic_der = False
    for offset, item in enumerate(raw):
        obj = _require_object(item, f"{label}.{key}[{offset}]")
        _reject_unknown_keys(obj, DER_OBJECT_KEYS, f"{label}.{key}[{offset}]")
        name = _optional_string(obj, "label", f"{label}.{key}[{offset}]")
        if name is not None:
            _reject_non_ascii_context(
                name,
                f"{label}.{key}[{offset}].label",
            )
            _reject_secret_looking_identifier(
                name,
                f"{label}.{key}[{offset}].label",
            )
            if len(name) > 128:
                raise TrustBundleError(f"{label}.{key}[{offset}].label must be no longer than 128 characters")
            if name in seen_labels:
                raise TrustBundleError(f"{label}.{key}[{offset}].label duplicates label")
            seen_labels.add(name)
        der_b64 = _required_string(
            obj,
            "der_base64",
            f"{label}.{key}[{offset}]",
            max_chars=None,
        )
        der, canonical_b64, is_synthetic_der = _strict_base64_der(
            der_b64,
            f"{label}.{key}[{offset}].der_base64",
            kind=kind,
            allow_synthetic_der=allow_synthetic_der,
        )
        uses_synthetic_der = uses_synthetic_der or is_synthetic_der
        digest = sha256_hex(der)
        declared_digest = obj.get("sha256")
        if not isinstance(declared_digest, str):
            raise TrustBundleError(f"{label}.{key}[{offset}].sha256 must be a string")
        if declared_digest != declared_digest.strip():
            raise TrustBundleError(
                f"{label}.{key}[{offset}].sha256 must not have surrounding whitespace"
            )
        if _validate_sha256(declared_digest, f"{label}.{key}[{offset}].sha256") != digest:
            raise TrustBundleError(f"{label}.{key}[{offset}].sha256 does not match der_base64")
        if digest in seen:
            raise TrustBundleError(f"{label}.{key}[{offset}] duplicates DER SHA-256")
        seen.add(digest)
        entry = {
            "sha256": digest,
            "der_base64": canonical_b64,
            "byte_len": len(der),
        }
        if name is not None:
            entry["label"] = name
        entries.append(entry)
    entries.sort(key=_der_summary_order_key)
    base64_values = [str(entry["der_base64"]) for entry in entries]
    return entries, base64_values, uses_synthetic_der


def _der_summary_order_key(entry: dict[str, Any]) -> tuple[str, int]:
    return str(entry["sha256"]), int(entry["byte_len"])


def _source(
    bundle: dict[str, Any],
    label: str,
    allow_insecure_source_url: bool,
) -> dict[str, Any]:
    if "source" not in bundle:
        raise TrustBundleError(f"{label}.source must be recorded")
    raw = bundle["source"]
    source = _require_object(raw, f"{label}.source")
    _reject_unknown_keys(source, SOURCE_KEYS, f"{label}.source")
    authority = _required_string(source, "authority", f"{label}.source")
    _reject_overlong_trust_source_text(authority, f"{label}.source.authority")
    _reject_non_ascii_context(authority, f"{label}.source.authority")
    _reject_secret_looking_identifier(authority, f"{label}.source.authority")
    version = _required_string(source, "version", f"{label}.source")
    _reject_overlong_trust_source_text(version, f"{label}.source.version")
    _reject_non_ascii_context(version, f"{label}.source.version")
    _reject_secret_looking_identifier(version, f"{label}.source.version")
    normalized: dict[str, Any] = {
        "authority": authority,
        "version": version,
    }
    retrieved_at = _required_string(source, "retrieved_at", f"{label}.source")
    _validate_retrieved_at(retrieved_at, f"{label}.source.retrieved_at")
    normalized["retrieved_at"] = retrieved_at
    url = _required_string(source, "url", f"{label}.source")
    _validate_source_url(
        url,
        f"{label}.source.url",
        allow_insecure_source_url=allow_insecure_source_url,
    )
    normalized["url"] = url
    return normalized


def _validate_retrieved_at(value: str, label: str) -> None:
    _parse_timestamp(value, label)


def _parse_timestamp(value: str, label: str) -> dt.datetime:
    if len(value) > MAX_TIMESTAMP_CHARS:
        raise TrustBundleError(
            f"{label} must be no longer than {MAX_TIMESTAMP_CHARS} characters"
        )
    if _has_unsafe_control(value):
        raise TrustBundleError(f"{label} must not contain control characters")
    normalized = value[:-1] + "+00:00" if value.endswith("Z") else value
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise TrustBundleError(f"{label} must be an ISO 8601 timestamp with timezone") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise TrustBundleError(f"{label} must include a timezone")
    parsed_utc = parsed.astimezone(dt.UTC)
    now = dt.datetime.now(dt.UTC)
    if parsed_utc > now + dt.timedelta(minutes=5):
        raise TrustBundleError(f"{label} must not be in the future")
    return parsed_utc


def _validate_source_url(
    url: str,
    label: str,
    *,
    allow_insecure_source_url: bool,
) -> None:
    if len(url) > MAX_SOURCE_URL_CHARS:
        raise TrustBundleError(f"{label} must be no longer than {MAX_SOURCE_URL_CHARS} characters")
    _reject_unsafe_control(url, label)
    _reject_url_percent_encoding_smuggling(url, label)
    if any(ch.isspace() for ch in url):
        raise TrustBundleError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise TrustBundleError(f"{label} is malformed") from error
    if parsed.scheme != "https" and not (parsed.scheme == "http" and allow_insecure_source_url):
        raise TrustBundleError(f"{label} must use HTTPS")
    try:
        port = parsed.port
    except ValueError as error:
        raise TrustBundleError(f"{label} has invalid port") from error
    port_text = _raw_url_port_text(parsed)
    if port_text == "":
        raise TrustBundleError(f"{label} must not include an empty port")
    if port_text is not None:
        if len(port_text) > 1 and port_text.startswith("0"):
            raise TrustBundleError(f"{label} port must not contain leading zeros")
        if port == 0:
            raise TrustBundleError(f"{label} port must be positive")
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise TrustBundleError(f"{label} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None:
        raise TrustBundleError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise TrustBundleError(f"{label} must not contain credentials")
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise TrustBundleError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise TrustBundleError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise TrustBundleError(f"{label} host must not end with a dot")
    if any(ord(ch) > 0x7E for ch in raw_host):
        raise TrustBundleError(f"{label} host must use printable ASCII")
    _reject_secret_looking_identifier(raw_host, f"{label} host")
    _validate_host_labels(raw_host, label)
    if parsed.params or parsed.query or parsed.fragment:
        raise TrustBundleError(f"{label} must not contain params, query, or fragment")
    _validate_url_path(parsed, label)
    hostname = hostname.strip().lower()
    if not hostname:
        raise TrustBundleError(f"{label} must include a host")
    if not allow_insecure_source_url:
        if hostname == "localhost" or hostname.endswith(".localhost"):
            raise TrustBundleError(f"{label} must not use localhost")
        if _host_uses_rebinding_suffix(hostname):
            raise TrustBundleError(f"{label} must not use local/private rebinding hostnames")
        try:
            address = ipaddress.ip_address(hostname)
        except ValueError:
            return
        if not address.is_global:
            raise TrustBundleError(f"{label} must not use local, private, or reserved IP addresses")
        if _address_embeds_non_global_ipv4(address):
            raise TrustBundleError(f"{label} must not embed local, private, or reserved IPv4 addresses")


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


def _trust_source_url_uses_placeholder_host(url: str) -> bool:
    parsed = urllib.parse.urlparse(url)
    hostname = (parsed.hostname or "").lower()
    return hostname in PLACEHOLDER_TRUST_SOURCE_HOSTS or any(
        hostname.endswith("." + host) for host in PLACEHOLDER_TRUST_SOURCE_HOSTS
    )


def _summary_source_has_placeholder(source: dict[str, Any]) -> bool:
    return (
        _trust_source_text_is_placeholder(source["authority"])
        or _trust_source_text_is_placeholder(source["version"])
        or _trust_source_url_uses_placeholder_host(source["url"])
    )


def _source_retrieved_at(source: dict[str, Any]) -> dt.datetime:
    return _parse_timestamp(source["retrieved_at"], "source.retrieved_at")


def _summary_source_is_stale(source: dict[str, Any], max_source_age_days: int) -> bool:
    retrieved_at = _source_retrieved_at(source)
    return retrieved_at < dt.datetime.now(dt.UTC) - dt.timedelta(days=max_source_age_days)


def _profile_json_emittable(args: argparse.Namespace, summaries: list[dict[str, Any]]) -> bool:
    return (
        not args.allow_synthetic_der
        and not args.allow_record_only
        and not args.allow_insecure_source_url
        and args.max_source_age_days is not None
        and not any(_summary_source_has_placeholder(summary["source"]) for summary in summaries)
        and not any(
            _summary_source_is_stale(summary["source"], args.max_source_age_days)
            for summary in summaries
        )
    )


def _summary_uses_insecure_source_url(summary: dict[str, Any]) -> bool:
    source = summary["source"]
    parsed = urllib.parse.urlparse(source["url"])
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


def _reject_unused_local_overrides(
    args: argparse.Namespace,
    summaries: list[dict[str, Any]],
) -> None:
    if args.allow_record_only and not any(
        summary["embedded_signature_policy"] != REQUIRE_VERIFIED
        for summary in summaries
    ):
        raise TrustBundleError(
            "--allow-record-only requires at least one bundle with a "
            "non-production embedded_signature_policy"
        )
    if args.allow_insecure_source_url and not any(
        _summary_uses_insecure_source_url(summary)
        for summary in summaries
    ):
        raise TrustBundleError(
            "--allow-insecure-source-url requires at least one bundle with an "
            "http:// or local/private source URL"
        )
    if args.allow_synthetic_der and not any(
        summary.get("_uses_synthetic_der") is True
        for summary in summaries
    ):
        raise TrustBundleError(
            "--allow-synthetic-der requires at least one bundle with synthetic DER"
        )


def _public_bundle_summary(summary: dict[str, Any]) -> dict[str, Any]:
    return {
        key: value
        for key, value in summary.items()
        if not key.startswith("_")
    }


def _bundle_summary_order_key(summary: dict[str, Any]) -> tuple[str, str, str]:
    return (
        str(summary["profile_id"]),
        str(summary["path"]),
        str(summary["bundle_sha256"]),
    )


def _reject_profile_emission_blockers(
    args: argparse.Namespace,
    summaries: list[dict[str, Any]],
) -> None:
    if args.emit_profile_json is None:
        return
    if args.allow_record_only:
        raise TrustBundleError(
            "--allow-record-only cannot be combined with --emit-profile-json; "
            "profile overrides require production signature policy"
        )
    if args.allow_insecure_source_url:
        raise TrustBundleError(
            "--allow-insecure-source-url cannot be combined with --emit-profile-json; "
            "profile overrides require production source provenance"
        )
    for offset, summary in enumerate(summaries):
        source = summary["source"]
        for field in ("authority", "version"):
            if _trust_source_text_is_placeholder(source[field]):
                raise TrustBundleError(
                    "cannot emit profile overrides from placeholder source metadata: "
                    f"bundles[{offset}].source.{field}"
                )
        if _trust_source_url_uses_placeholder_host(source["url"]):
            raise TrustBundleError(
                "cannot emit profile overrides from reserved placeholder source provenance: "
                f"bundles[{offset}].source.url"
            )
    if args.max_source_age_days is None:
        raise TrustBundleError("--max-source-age-days is required with --emit-profile-json")
    for offset, summary in enumerate(summaries):
        source = summary["source"]
        if _summary_source_is_stale(source, args.max_source_age_days):
            raise TrustBundleError(
                f"bundles[{offset}].source.retrieved_at is older than the "
                f"{args.max_source_age_days}-day freshness budget"
            )


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


def _validate_host_labels(raw_host: str, label: str) -> None:
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise TrustBundleError(f"{label} host must be a valid IP address")
    if len(raw_host) > 253:
        raise TrustBundleError(f"{label} host must be at most 253 characters")
    _reject_legacy_ipv4_host_notation(raw_host, label)
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise TrustBundleError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise TrustBundleError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise TrustBundleError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise TrustBundleError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise TrustBundleError(
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
        raise TrustBundleError(f"{label} host must not use legacy IPv4 numeric notation")


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if any(ord(ch) > 0x7E for ch in path):
        raise TrustBundleError(f"{label} path must use printable ASCII")
    if "\\" in path:
        raise TrustBundleError(f"{label} path must use forward slashes")
    if ";" in path:
        raise TrustBundleError(f"{label} path must not contain semicolon parameters")
    if any(token in path for token in (":", "@", "[", "]")):
        raise TrustBundleError(f"{label} path must not contain URL delimiter characters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise TrustBundleError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise TrustBundleError(f"{label} path must not contain dot segments")
    if _contains_secret_material(path) or _contains_secret_identifier_material(path):
        raise TrustBundleError(f"{label} path must not contain secret-looking material")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise TrustBundleError(f"{label} path must not contain encoded dot or separator characters")
    if "%3b" in lowered:
        raise TrustBundleError(f"{label} path must not contain encoded semicolon parameters")
    if any(token in lowered for token in ("%23", "%3a", "%3f", "%40", "%5b", "%5d")):
        raise TrustBundleError(f"{label} path must not contain encoded URL delimiter characters")
    if "%25" in lowered:
        raise TrustBundleError(f"{label} path must not contain encoded percent characters")
    if re.search(r"%[89a-f][0-9a-f]", lowered):
        raise TrustBundleError(
            f"{label} path must not contain percent-encoded non-ASCII bytes"
        )


def _merge_unique(values: list[str], additions: list[str], label: str) -> list[str]:
    seen = set(values)
    result = list(values)
    for value in additions:
        if value in seen:
            raise TrustBundleError(f"{label} duplicates SHA-256")
        seen.add(value)
        result.append(value)
    return sorted(result)


def _reject_overlap(left: list[str], right: list[str], label: str) -> None:
    overlap = sorted(set(left) & set(right))
    if overlap:
        raise TrustBundleError(f"{label} contains conflicting SHA-256")


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path)
        if key in seen:
            raise TrustBundleError(f"{label}[{offset}] duplicates {label}[{seen[key]}]")
        seen[key] = offset


def _reject_duplicate_summary_field(
    summaries: list[dict[str, Any]],
    field: str,
    label: str,
) -> None:
    seen: dict[str, int] = {}
    for offset, summary in enumerate(summaries):
        value = summary[field]
        if value in seen:
            raise TrustBundleError(
                f"{label}[{offset}].{field} duplicates "
                f"{label}[{seen[value]}].{field}"
            )
        seen[value] = offset


def verify_bundle(
    path: Path,
    *,
    allow_record_only: bool,
    allow_insecure_source_url: bool,
    allow_synthetic_der: bool,
    display_label: str | None = None,
) -> dict[str, Any]:
    """Verify one trust bundle and return a normalized summary."""

    label = display_label or "trust bundle"
    bundle = _require_object(_load_json(path, display_label=label), label)
    _reject_unknown_keys(bundle, TOP_LEVEL_KEYS, label)
    _check_no_secret_material(bundle, label)
    bundle_version = bundle.get("version")
    if (
        isinstance(bundle_version, bool)
        or not isinstance(bundle_version, int)
        or bundle_version != BUNDLE_VERSION
    ):
        raise TrustBundleError(f"{label}.version must be {BUNDLE_VERSION}")

    profile_id = _required_profile_id(bundle, "profile_id", label)
    rail = _required_rail(bundle, "rail", label)
    environment = _required_context_string(bundle, "environment", label)
    if "embedded_signature_policy" not in bundle:
        raise TrustBundleError(f"{label}.embedded_signature_policy must be recorded")
    policy = _required_string(bundle, "embedded_signature_policy", label)
    _reject_overlong_trust_policy(policy, f"{label}.embedded_signature_policy")
    _reject_non_ascii_context(policy, f"{label}.embedded_signature_policy")
    _reject_secret_looking_identifier(policy, f"{label}.embedded_signature_policy")
    if not isinstance(policy, str) or policy not in POLICIES:
        raise TrustBundleError(f"{label}.embedded_signature_policy is unsupported")
    if policy != REQUIRE_VERIFIED and not allow_record_only:
        raise TrustBundleError(
            f"{label}.embedded_signature_policy must be {REQUIRE_VERIFIED!r} for production bundles"
        )

    raw_public_pins = _sha256_list(bundle, "signature_public_key_sha256_pins", label)
    legacy_public_pins = _sha256_list(bundle, "trusted_public_key_sha256", label)
    anchor_pin_values = _sha256_list(bundle, "x509_trust_anchor_sha256_pins", label)
    legacy_anchor_pin_values = _sha256_list(bundle, "trusted_certificate_sha256", label)
    revoked_pin_values = _sha256_list(bundle, "revoked_certificate_sha256", label)
    policy_oids = _oid_list(bundle, "x509_required_certificate_policy_oids", label)

    trust_anchors, trust_anchor_der_values, trust_anchor_uses_synthetic_der = _der_objects(
        bundle,
        "x509_trust_anchors",
        label,
        kind=DER_KIND_CERTIFICATE,
        allow_synthetic_der=allow_synthetic_der,
    )
    revoked_certificates, _revoked_der_values, revoked_uses_synthetic_der = _der_objects(
        bundle,
        "revoked_certificates",
        label,
        kind=DER_KIND_CERTIFICATE,
        allow_synthetic_der=allow_synthetic_der,
    )
    crls, crl_values, crl_uses_synthetic_der = _der_objects(
        bundle,
        "x509_crls",
        label,
        kind=DER_KIND_CRL,
        allow_synthetic_der=allow_synthetic_der,
    )
    ocsp_responses, ocsp_values, ocsp_uses_synthetic_der = _der_objects(
        bundle,
        "x509_ocsp_responses",
        label,
        kind=DER_KIND_OCSP,
        allow_synthetic_der=allow_synthetic_der,
    )

    x509_trust_anchor_sha256_pins = _merge_unique(
        anchor_pin_values,
        [entry["sha256"] for entry in trust_anchors],
        f"{label}.x509_trust_anchor_sha256_pins",
    )
    trusted_certificate_sha256 = _merge_unique(
        legacy_anchor_pin_values,
        [],
        f"{label}.trusted_certificate_sha256",
    )
    revoked_certificate_sha256 = _merge_unique(
        revoked_pin_values,
        [entry["sha256"] for entry in revoked_certificates],
        f"{label}.revoked_certificate_sha256",
    )
    _reject_overlap(
        raw_public_pins,
        legacy_public_pins,
        f"{label}.signature_public_key_sha256_pins/trusted_public_key_sha256",
    )
    _reject_overlap(
        x509_trust_anchor_sha256_pins,
        trusted_certificate_sha256,
        f"{label}.x509_trust_anchor_sha256_pins/trusted_certificate_sha256",
    )
    _reject_overlap(
        x509_trust_anchor_sha256_pins + trusted_certificate_sha256,
        revoked_certificate_sha256,
        f"{label}.trusted/revoked certificate pins",
    )
    _reject_overlap(
        raw_public_pins + legacy_public_pins,
        x509_trust_anchor_sha256_pins
        + trusted_certificate_sha256
        + revoked_certificate_sha256,
        f"{label}.public-key/certificate SHA-256 pins",
    )
    _reject_overlap(
        raw_public_pins
        + legacy_public_pins
        + x509_trust_anchor_sha256_pins
        + trusted_certificate_sha256
        + revoked_certificate_sha256,
        [entry["sha256"] for entry in crls]
        + [entry["sha256"] for entry in ocsp_responses],
        f"{label}.trust pin/revocation DER SHA-256 roles",
    )

    crl_required = _required_bool(bundle, "x509_require_crl_revocation_check", label)
    ocsp_required = _required_bool(bundle, "x509_require_ocsp_revocation_check", label)
    if crl_required and not crl_values:
        raise TrustBundleError(f"{label} requires CRL revocation checking but has no x509_crls")
    if ocsp_required and not ocsp_values:
        raise TrustBundleError(f"{label} requires OCSP revocation checking but has no x509_ocsp_responses")
    if policy == REQUIRE_VERIFIED and not (
        raw_public_pins
        or legacy_public_pins
        or x509_trust_anchor_sha256_pins
        or trusted_certificate_sha256
    ):
        raise TrustBundleError(f"{label} has require-verified policy but no trust pins")

    source = _source(bundle, label, allow_insecure_source_url)
    profile_overrides = {
        "id": profile_id,
        "rail": rail,
        "embedded_signature_policy": policy,
        "signature_public_key_sha256_pins": raw_public_pins,
        "trusted_public_key_sha256": legacy_public_pins,
        "x509_trust_anchor_sha256_pins": x509_trust_anchor_sha256_pins,
        "trusted_certificate_sha256": trusted_certificate_sha256,
        "revoked_certificate_sha256": revoked_certificate_sha256,
        "x509_required_certificate_policy_oids": policy_oids,
        "x509_require_crl_revocation_check": crl_required,
        "x509_crl_der_base64": crl_values,
        "x509_require_ocsp_revocation_check": ocsp_required,
        "x509_ocsp_response_der_base64": ocsp_values,
    }
    material_summary = {
        "signature_public_key_pin_count": len(raw_public_pins) + len(legacy_public_pins),
        "x509_trust_anchor_pin_count": len(x509_trust_anchor_sha256_pins)
        + len(trusted_certificate_sha256),
        "revoked_certificate_pin_count": len(revoked_certificate_sha256),
        "x509_crl_count": len(crls),
        "x509_ocsp_response_count": len(ocsp_responses),
        "x509_required_certificate_policy_oid_count": len(policy_oids),
    }
    summary = {
        "path": str(path),
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "source": source,
        "embedded_signature_policy": policy,
        "material": material_summary,
        "x509_trust_anchors": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in trust_anchors
        ],
        "revoked_certificates": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in revoked_certificates
        ],
        "x509_crls": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in crls
        ],
        "x509_ocsp_responses": [
            {key: value for key, value in entry.items() if key != "der_base64"}
            for entry in ocsp_responses
        ],
        "profile_overrides": profile_overrides,
    }
    summary["_uses_synthetic_der"] = (
        trust_anchor_uses_synthetic_der
        or revoked_uses_synthetic_der
        or crl_uses_synthetic_der
        or ocsp_uses_synthetic_der
    )
    summary["bundle_sha256"] = sha256_hex(_canonical_json_bytes(bundle))
    return summary


def run(args: argparse.Namespace) -> int:
    args.summary_out = _optional_cli_path(getattr(args, "summary_out", None), "summary_out")
    args.emit_profile_json = _optional_cli_path(
        getattr(args, "emit_profile_json", None),
        "emit_profile_json",
    )
    if args.summary_out is not None:
        _reject_output_path_smuggling(args.summary_out, "summary_out")
        _reject_repository_output_path(args.summary_out, "summary_out")
    if args.emit_profile_json is not None:
        _reject_output_path_smuggling(args.emit_profile_json, "emit_profile_json")
        _reject_repository_output_path(args.emit_profile_json, "emit_profile_json")
    bundle_paths = _required_cli_path_sequence(getattr(args, "bundle", None), "--bundle")
    for offset, path in enumerate(bundle_paths):
        _reject_output_path_smuggling(path, f"--bundle[{offset}]")
    _require_policy_booleans(args)
    if not bundle_paths:
        raise TrustBundleError("provide at least one --bundle")
    args.max_source_age_days = _optional_positive_cli_int(
        getattr(args, "max_source_age_days", None),
        "--max-source-age-days",
    )
    if args.allow_synthetic_der and args.emit_profile_json is not None:
        raise TrustBundleError(
            "--allow-synthetic-der cannot be combined with --emit-profile-json; "
            "replace template DER with real rail material before emitting profile overrides"
    )
    bundle_inputs = tuple(
        (f"--bundle[{offset}]", path) for offset, path in enumerate(bundle_paths)
    )
    _reject_output_output_alias(
        args.summary_out,
        "--summary-out",
        args.emit_profile_json,
        "--emit-profile-json",
    )
    _reject_output_input_alias(args.summary_out, "summary_out", bundle_inputs)
    _reject_output_input_alias(args.emit_profile_json, "emit_profile_json", bundle_inputs)
    if args.summary_out is not None:
        _ensure_text_output_target(
            args.summary_out,
            display_label="summary_out",
            create_parent=False,
        )
    if args.emit_profile_json is not None:
        _ensure_text_output_target(
            args.emit_profile_json,
            display_label="emit_profile_json",
            create_parent=False,
        )
    _reject_duplicate_paths([path.resolve() for path in bundle_paths], "--bundle")
    summaries = []
    for offset, path in enumerate(bundle_paths):
        summaries.append(
            verify_bundle(
                path,
                allow_record_only=args.allow_record_only,
                allow_insecure_source_url=args.allow_insecure_source_url,
                allow_synthetic_der=args.allow_synthetic_der,
                display_label=f"bundle[{offset}]",
            )
        )
    _reject_profile_emission_blockers(args, summaries)
    _reject_unused_local_overrides(args, summaries)
    _reject_duplicate_summary_field(summaries, "bundle_sha256", "bundles")
    _reject_duplicate_summary_field(summaries, "profile_id", "bundles")
    profile_json_emittable = _profile_json_emittable(args, summaries)
    canonical_summaries = sorted(summaries, key=_bundle_summary_order_key)
    public_summaries = [
        _public_bundle_summary(summary) for summary in canonical_summaries
    ]
    profile_text = None
    profile_json_sha256 = None
    if args.emit_profile_json is not None:
        profile_config = [
            summary["profile_overrides"] for summary in canonical_summaries
        ]
        profile_text = json.dumps(profile_config, indent=2, sort_keys=True) + "\n"
        profile_json_sha256 = sha256_hex(profile_text.encode("utf-8"))
    output: dict[str, Any] = {
        "version": TRUST_SUMMARY_VERSION,
        "verified_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "verified_bundles": len(summaries),
        "allow_record_only": args.allow_record_only,
        "allow_insecure_source_url": args.allow_insecure_source_url,
        "allow_synthetic_der": args.allow_synthetic_der,
        "max_source_age_days": args.max_source_age_days,
        "profile_json_emitted": args.emit_profile_json is not None,
        "profile_json_emittable": profile_json_emittable,
        "profile_json_sha256": profile_json_sha256,
        "bundles": public_summaries,
    }
    output[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_json_bytes(output))
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    if profile_text is not None:
        _write_text_output(
            args.emit_profile_json,
            profile_text,
            display_label="emit_profile_json",
        )
    if args.summary_out is not None:
        _write_text_output(args.summary_out, text, display_label="summary_out")
    print(text, end="")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 XMLDSig/XAdES operator trust bundle JSON.",
        allow_abbrev=False,
    )
    parser.add_argument(
        "--bundle",
        action="append",
        required=True,
        type=Path,
        help="Trust bundle JSON file to verify; repeatable.",
    )
    parser.add_argument(
        "--summary-out",
        type=Path,
        help="Optional path to write the verification summary JSON.",
    )
    parser.add_argument(
        "--emit-profile-json",
        type=Path,
        help="Optional path to write Torii profile trust override JSON.",
    )
    parser.add_argument(
        "--allow-record-only",
        action="store_true",
        help="Allow non-production record-only or reject-unsupported policies.",
    )
    parser.add_argument(
        "--allow-insecure-source-url",
        action="store_true",
        help="Allow http:// provenance URLs for local tests.",
    )
    parser.add_argument(
        "--allow-synthetic-der",
        action="store_true",
        help="Allow DER SEQUENCE placeholders for checked-in templates; production bundles should omit this.",
    )
    parser.add_argument(
        "--max-source-age-days",
        help=(
            "Maximum age in days for source.retrieved_at when deciding whether "
            "profile override JSON can be emitted."
        ),
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    try:
        _preflight_raw_cli_secrets(
            argv,
            {
                "--bundle",
                "--emit-profile-json",
                "--max-source-age-days",
                "--summary-out",
            },
        )
        _preflight_boolean_cli_flags(
            argv,
            {
                "--allow-insecure-source-url",
                "--allow-record-only",
                "--allow-synthetic-der",
            },
        )
        _preflight_positive_int_cli_values(argv, {"--max-source-age-days"})
        _preflight_output_cli_paths(
            argv,
            {"--bundle", "--emit-profile-json", "--summary-out"},
        )
        args = parser.parse_args(argv)
        return run(args)
    except TrustBundleError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
