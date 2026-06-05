#!/usr/bin/env python3
"""Aggregate ISO 20022 production-readiness evidence.

Purpose:
  This offline release gate combines the ISO XSD fixture preflight and operator
  production-evidence summaries into one digest-bound readiness report. It
  fails closed on missing strict XSD closure, local-test evidence overrides,
  plan-only or partial canary evidence, weak receipt evidence, non-production
  trust policy, and provider/environment drift.

Prerequisites:
  Python 3.11+. No third party Python packages are required. XSD summaries
  should come from ``iso_xsd_fixture_verify.py`` and evidence summaries should
  come from ``iso_operator_evidence_verify.py``.

Safety:
  The script is read-only unless ``--summary-out`` is supplied. It does not
  contact Torii, rail gateways, notaries, PKI endpoints, OCSP, CRL, or remote
  schema repositories.
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
import urllib.parse
from pathlib import Path
from typing import Any


READINESS_VERSION = 1
EVIDENCE_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
MAX_TRUST_DER_BLOBS = 8
MAX_TRUST_DER_BYTES = 1024 * 1024
MESSAGE_DEF_ID_RE = re.compile(r"^[a-z]{4}\.\d{3}\.\d{3}\.\d{2}$")
PROFILE_ID_RE = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")
MESSAGE_TYPE_RE = re.compile(r"^[a-z]{4}\.\d{3}$")
SOURCE_COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
SOURCE_REPOSITORY_RE = re.compile(
    r"^https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$"
)
PROFILE_DIRECTIONS = {"inbound", "outbound", "follow-up"}
REQUIRED_CANARY_STAGES = {"rail", "notary", "verify"}
REQUIRED_RECEIPT_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
REQUIRE_VERIFIED = "require-verified"
RECEIPT_PATH_SUFFIX = ".receipt.json"
ALLOWED_SCHEMA_SOURCE_LICENSES = {"Apache-2.0"}
SCHEMA_SOURCE_KEYS = {"repository", "commit", "path", "license", "sha256"}
TRUST_SOURCE_KEYS = {"authority", "version", "url", "retrieved_at"}
TRUST_DER_PROOF_KEYS = {"sha256", "byte_len"}
PRODUCTION_FALSE_POLICY_FLAGS = {
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
}
EVIDENCE_FRESHNESS_POLICY_FIELDS = {
    "max_canary_age_days",
    "max_trust_age_days",
    "max_trust_source_age_days",
}
EVIDENCE_SUMMARY_KEYS = {
    "version",
    "verified_at",
    "ok",
    "canary_summaries",
    "trust_summaries",
    "receipt_verification",
    "policy",
    SUMMARY_DIGEST_FIELD,
}
EVIDENCE_POLICY_KEYS = {
    "provider",
    "environment",
    *PRODUCTION_FALSE_POLICY_FLAGS,
    *EVIDENCE_FRESHNESS_POLICY_FIELDS,
}
COMPACT_CANARY_KEYS = {
    "path",
    "config_path",
    "provider",
    "environment",
    "started_at",
    "finished_at",
    "plan_only",
    "require_explicit_policy",
    "stage_names",
    "stage_windows",
    "receipt_summary",
    SUMMARY_DIGEST_FIELD,
}
COMPACT_STAGE_WINDOW_KEYS = {"name", "started_at", "finished_at"}
RECEIPT_SUMMARY_KEYS = {
    "verified_receipts",
    "receipt_kind",
    "allow_failed",
    "allow_insecure_http",
    "allow_legacy_colr007",
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
}
TRUST_SUMMARY_KEYS = {
    "path",
    "verified_at",
    "verified_bundles",
    "profile_json_emitted",
    "profile_json_emittable",
    "profile_json_sha256",
    "profiles",
    SUMMARY_DIGEST_FIELD,
}
TRUST_PROFILE_KEYS = {
    "profile_id",
    "rail",
    "environment",
    "bundle_sha256",
    "source",
    "embedded_signature_policy",
    "signature_public_key_pin_count",
    "x509_trust_anchor_pin_count",
    "x509_trust_anchor_der",
    "revoked_certificate_pin_count",
    "revoked_certificate_der",
    "x509_required_certificate_policy_oid_count",
    "x509_require_crl_revocation_check",
    "x509_crl_count",
    "x509_crl_der",
    "x509_require_ocsp_revocation_check",
    "x509_ocsp_response_count",
    "x509_ocsp_response_der",
}
XSD_SUMMARY_KEYS = {
    "verified_at",
    "manifest",
    "manifest_sha256",
    "verified_schemas",
    "verified_fixtures",
    "schema_backed_fixtures",
    "schema_validated_fixtures",
    "profile_checked_versions",
    "profile_schema_backed_versions",
    "missing_schema_fixtures",
    "schema_only_entries",
    "missing_profile_schema_versions",
    "schemas",
    "fixtures",
    "profile_catalog",
    "strict",
    SUMMARY_DIGEST_FIELD,
}
XSD_SCHEMA_KEYS = {
    "path",
    "message_def_id",
    "target_namespace",
    "payload_root",
    "sha256",
    "schema_only",
    "schema_only_reason",
    "source",
}
XSD_FIXTURE_KEYS = {
    "path",
    "message_def_id",
    "payload_root",
    "sha256",
    "schema",
    "schema_backed",
    "schema_validated",
    "missing_schema_reason",
}
XSD_GAP_ENTRY_KEYS = {"path", "message_def_id", "reason"}
XSD_STRICT_KEYS = {
    "require_schema_backed_fixtures",
    "require_fixture_for_schema",
    "require_profile_schema_backed_versions",
    "validate_xml_schema",
}
XSD_PROFILE_CATALOG_KEYS = {
    "path",
    "sha256",
    "catalog_json_sha256",
    "profiles",
    "checked_versions",
    "schema_backed_versions",
    "missing_schema_versions",
    "skipped_family_versions",
    "versions",
}
XSD_PROFILE_VERSION_KEYS = {
    "profile_id",
    "message_type",
    "direction",
    "message_def_id",
    "schema_backed",
}
XSD_PROFILE_MISSING_VERSION_KEYS = {
    "profile_id",
    "message_type",
    "direction",
    "message_def_id",
}
XSD_PROFILE_SKIPPED_VERSION_KEYS = {
    "profile_id",
    "message_type",
    "direction",
    "version",
}


class ReadinessError(RuntimeError):
    """Raised when a readiness input is malformed or digest-tampered."""


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


def _read_regular_file(path: Path) -> bytes:
    try:
        mode = path.lstat().st_mode
    except FileNotFoundError as error:
        raise ReadinessError(f"{path} does not exist") from error
    if stat.S_ISLNK(mode):
        raise ReadinessError(f"{path} must not be a symlink")
    if not stat.S_ISREG(mode):
        raise ReadinessError(f"{path} must be a regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ReadinessError(f"{path} must be a regular file")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            return handle.read()
    except FileNotFoundError as error:
        raise ReadinessError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise ReadinessError(f"{path} must not be a symlink") from error
        raise ReadinessError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _write_text_output(path: Path, text: str) -> None:
    try:
        path.parent.mkdir(parents=True, exist_ok=True)
    except FileExistsError as error:
        raise ReadinessError(f"{path.parent} must be a directory") from error
    parent_mode = path.parent.lstat().st_mode
    if stat.S_ISLNK(parent_mode):
        raise ReadinessError(f"{path.parent} must not be a symlink")
    if not stat.S_ISDIR(parent_mode):
        raise ReadinessError(f"{path.parent} must be a directory")
    if path.exists() or path.is_symlink():
        mode = path.lstat().st_mode
        if stat.S_ISLNK(mode):
            raise ReadinessError(f"{path} must not be a symlink")
        if not stat.S_ISREG(mode):
            raise ReadinessError(f"{path} must be a regular file")
    parent_flags = os.O_RDONLY | getattr(os, "O_DIRECTORY", 0) | getattr(os, "O_CLOEXEC", 0)
    nofollow = getattr(os, "O_NOFOLLOW", 0)
    try:
        parent_fd = os.open(path.parent, parent_flags | nofollow)
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise ReadinessError(f"{path.parent} must not be a symlink") from error
        raise ReadinessError(f"{path.parent} must be a directory") from error

    fd = -1
    try:
        flags = os.O_WRONLY | os.O_CREAT | os.O_TRUNC | getattr(os, "O_CLOEXEC", 0)
        try:
            fd = os.open(path.name, flags | nofollow, 0o666, dir_fd=parent_fd)
        except OSError as error:
            if error.errno == errno.ELOOP:
                raise ReadinessError(f"{path} must not be a symlink") from error
            raise ReadinessError(f"cannot open {path} for writing: {error.strerror}") from error
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ReadinessError(f"{path} must be a regular file")
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            fd = -1
            handle.write(text)
    finally:
        if fd >= 0:
            os.close(fd)
        os.close(parent_fd)


def _load_json(path: Path) -> Any:
    try:
        raw = _read_regular_file(path)
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise ReadinessError(f"{path} is not UTF-8 JSON") from error
    try:
        return json.loads(
            text,
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except json.JSONDecodeError as error:
        raise ReadinessError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ReadinessError(f"duplicate key {key!r} in JSON object")
        result[key] = value
    return result


def _require_object(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReadinessError(f"{label} must be a JSON object")
    return value


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise ReadinessError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        raise ReadinessError(f"{label} must be a JSON array")
    return value


def _require_string(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not isinstance(raw, str) or not raw.strip():
        raise ReadinessError(f"{label}.{key} must be a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label}.{key} must not contain control characters")
    if raw != raw.strip():
        raise ReadinessError(f"{label}.{key} must not have surrounding whitespace")
    return raw


def _require_cli_string(value: str | None, label: str) -> str:
    if value is None or not value.strip():
        raise ReadinessError(f"provide {label}")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise ReadinessError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReadinessError(f"{label} must not have surrounding whitespace")
    return value


def _require_positive_cli_int(value: int | None, label: str) -> int:
    if value is None:
        raise ReadinessError(f"provide {label}")
    if isinstance(value, bool) or not isinstance(value, int) or value <= 0:
        raise ReadinessError(f"{label} must be a positive integer")
    return value


def _reject_duplicate_paths(paths: list[Path], label: str) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path)
        if key in seen:
            raise ReadinessError(
                f"{label}[{offset}] duplicates {label}[{seen[key]}]: {key}"
            )
        seen[key] = offset


def _reject_duplicate_compact_summaries(
    summaries: list[dict[str, Any]],
    label: str,
) -> None:
    seen_paths: dict[str, int] = {}
    seen_digests: dict[str, int] = {}
    for offset, summary in enumerate(summaries):
        path = summary["path"]
        if path in seen_paths:
            raise ReadinessError(
                f"{label}[{offset}].path duplicates {label}[{seen_paths[path]}].path: {path}"
            )
        seen_paths[path] = offset
        digest = summary[SUMMARY_DIGEST_FIELD]
        if digest in seen_digests:
            raise ReadinessError(
                f"{label}[{offset}].{SUMMARY_DIGEST_FIELD} duplicates "
                f"{label}[{seen_digests[digest]}].{SUMMARY_DIGEST_FIELD}: {digest}"
            )
        seen_digests[digest] = offset


def _require_bool(value: dict[str, Any], key: str, label: str) -> bool:
    raw = value.get(key)
    if not isinstance(raw, bool):
        raise ReadinessError(f"{label}.{key} must be a boolean")
    return raw


def _require_positive_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw <= 0:
        raise ReadinessError(f"{label}.{key} must be a positive integer")
    return raw


def _require_nonnegative_int(value: dict[str, Any], key: str, label: str) -> int:
    raw = value.get(key)
    if isinstance(raw, bool) or not isinstance(raw, int) or raw < 0:
        raise ReadinessError(f"{label}.{key} must be a non-negative integer")
    return raw


def _is_lower_sha256(value: Any) -> bool:
    return (
        isinstance(value, str)
        and len(value) == 64
        and all(ch in "0123456789abcdef" for ch in value)
    )


def _require_sha256(value: dict[str, Any], key: str, label: str) -> str:
    raw = value.get(key)
    if not _is_lower_sha256(raw):
        raise ReadinessError(f"{label}.{key} must be a lowercase SHA-256 digest")
    return raw


def _require_compact_der_entries(
    value: dict[str, Any],
    key: str,
    label: str,
) -> list[dict[str, int | str]]:
    items = _require_list(value.get(key), f"{label}.{key}")
    if len(items) > MAX_TRUST_DER_BLOBS:
        raise ReadinessError(
            f"{label}.{key} must not contain more than {MAX_TRUST_DER_BLOBS} entries"
        )
    result: list[dict[str, int | str]] = []
    seen: dict[str, int] = {}
    for offset, raw_entry in enumerate(items):
        entry_label = f"{label}.{key}[{offset}]"
        entry = _require_object(raw_entry, entry_label)
        _reject_unknown_keys(entry, TRUST_DER_PROOF_KEYS, entry_label)
        digest = _require_sha256(entry, "sha256", entry_label)
        if digest in seen:
            raise ReadinessError(
                f"{entry_label}.sha256 duplicates {label}.{key}[{seen[digest]}].sha256"
            )
        seen[digest] = offset
        byte_len = _require_positive_int(entry, "byte_len", entry_label)
        if byte_len > MAX_TRUST_DER_BYTES:
            raise ReadinessError(
                f"{entry_label}.byte_len must be no more than {MAX_TRUST_DER_BYTES}"
            )
        result.append({"sha256": digest, "byte_len": byte_len})
    return result


def _validate_receipt_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    if not raw.endswith(RECEIPT_PATH_SUFFIX):
        raise ReadinessError(f"{label} must point to a {RECEIPT_PATH_SUFFIX} file")
    return raw


def _validate_compact_summary_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    return raw


def _validate_config_path(raw: str, label: str) -> str:
    _reject_path_smuggling(raw, label)
    if not raw.endswith(".json"):
        raise ReadinessError(f"{label} must point to a .json file")
    return raw


def _reject_path_smuggling(raw: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise ReadinessError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise ReadinessError(f"{label} must not contain whitespace")
    if ";" in raw:
        raise ReadinessError(f"{label} must not contain semicolon path parameters")
    parts = raw.split("/")
    checked_parts = parts[1:] if raw.startswith("/") else parts
    if any(part == "" for part in checked_parts):
        raise ReadinessError(f"{label} must not contain empty path segments")
    normalized_parts = [part for part in raw.replace("\\", "/").split("/") if part]
    if any(part in {".", ".."} for part in normalized_parts):
        raise ReadinessError(f"{label} must not contain dot or parent segments")
    if "\\" in raw:
        raise ReadinessError(f"{label} must use forward slashes")


def _require_message_def_id(value: dict[str, Any], key: str, label: str) -> str:
    raw = _require_string(value, key, label)
    if MESSAGE_DEF_ID_RE.fullmatch(raw) is None:
        raise ReadinessError(f"{label}.{key} must be a lowercase ISO message id")
    return raw


def _require_profile_id(value: dict[str, Any], key: str, label: str) -> str:
    raw = _require_string(value, key, label)
    if PROFILE_ID_RE.fullmatch(raw) is None:
        raise ReadinessError(f"{label}.{key} must be a canonical lowercase profile id")
    return raw


def _require_message_type(value: dict[str, Any], key: str, label: str) -> str:
    raw = _require_string(value, key, label)
    if MESSAGE_TYPE_RE.fullmatch(raw) is None:
        raise ReadinessError(f"{label}.{key} must be lowercase ISO family id")
    return raw


def _require_profile_direction(value: dict[str, Any], key: str, label: str) -> str:
    raw = _require_string(value, key, label)
    if raw not in PROFILE_DIRECTIONS:
        raise ReadinessError(
            f"{label}.{key} must be one of " + ", ".join(sorted(PROFILE_DIRECTIONS))
        )
    return raw


def _validate_schema_source_path(raw: str, label: str) -> str:
    if "\\" in raw:
        raise ReadinessError(f"{label} must use forward slashes")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise ReadinessError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise ReadinessError(f"{label} must not contain whitespace")
    if ";" in raw:
        raise ReadinessError(f"{label} must not contain semicolon path parameters")
    path = Path(raw)
    if path.is_absolute():
        raise ReadinessError(f"{label} must be relative, got {raw}")
    if not raw.endswith(".xsd"):
        raise ReadinessError(f"{label} must point to an .xsd file")
    if any(part in {"", ".", ".."} for part in raw.split("/")):
        raise ReadinessError(f"{label} must not contain empty, dot, or parent segments")
    return raw


def _validate_fixture_summary_path(raw: str, label: str) -> str:
    if "\\" in raw:
        raise ReadinessError(f"{label} must use forward slashes")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise ReadinessError(f"{label} must not have surrounding whitespace")
    if any(ch.isspace() for ch in raw):
        raise ReadinessError(f"{label} must not contain whitespace")
    if ";" in raw:
        raise ReadinessError(f"{label} must not contain semicolon path parameters")
    if Path(raw).is_absolute():
        raise ReadinessError(f"{label} must be relative, got {raw}")
    if not raw.endswith(".xml"):
        raise ReadinessError(f"{label} must point to an .xml file")
    parts = raw.split("/")
    if any(part in {"", "."} for part in parts):
        raise ReadinessError(f"{label} must not contain empty or dot segments")
    seen_child_segment = False
    for part in parts:
        if part == "..":
            if seen_child_segment:
                raise ReadinessError(f"{label} parent segments must be leading")
        else:
            seen_child_segment = True
    return raw


def _validate_reviewed_gap_reason(raw: Any, label: str) -> str | None:
    if raw is None:
        return None
    if not isinstance(raw, str) or not raw.strip():
        raise ReadinessError(f"{label} must be a non-empty string when provided")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label} must not contain control characters")
    if raw != raw.strip():
        raise ReadinessError(f"{label} must not have surrounding whitespace")
    return raw


def _verify_schema_source_summary(
    source_raw: Any,
    label: str,
    *,
    message_def_id: str,
    schema_sha256: str,
    blockers: list[dict[str, Any]],
    path: Path,
) -> dict[str, str]:
    source = _require_object(source_raw, label)
    _reject_unknown_keys(source, SCHEMA_SOURCE_KEYS, label)
    repository = _require_string(source, "repository", label)
    if SOURCE_REPOSITORY_RE.fullmatch(repository) is None or repository.endswith(".git"):
        _blocker(
            blockers,
            "xsd.schema_source_repository_invalid",
            f"{label}.repository is not a canonical GitHub source URL",
            path,
        )
    commit = _require_string(source, "commit", label)
    if SOURCE_COMMIT_RE.fullmatch(commit) is None:
        _blocker(
            blockers,
            "xsd.schema_source_commit_invalid",
            f"{label}.commit is not a lowercase 40-hex Git commit",
            path,
        )
    source_path = _validate_schema_source_path(_require_string(source, "path", label), f"{label}.path")
    if Path(source_path).name != f"{message_def_id}.xsd":
        _blocker(
            blockers,
            "xsd.schema_source_path_mismatch",
            f"{label}.path filename does not match message_def_id",
            path,
        )
    license_id = _require_string(source, "license", label)
    if license_id not in ALLOWED_SCHEMA_SOURCE_LICENSES:
        _blocker(
            blockers,
            "xsd.schema_source_license_invalid",
            f"{label}.license is not an allowed redistributable source license",
            path,
        )
    source_sha256 = _require_sha256(source, "sha256", label)
    if source_sha256 != schema_sha256:
        _blocker(
            blockers,
            "xsd.schema_source_digest_mismatch",
            f"{label}.sha256 does not match the schema digest",
            path,
        )
    return {
        "repository": repository,
        "commit": commit,
        "path": source_path,
        "license": license_id,
        "sha256": source_sha256,
    }


def _parse_timestamp(raw: str, label: str) -> dt.datetime:
    normalized = raw[:-1] + "+00:00" if raw.endswith("Z") else raw
    try:
        parsed = dt.datetime.fromisoformat(normalized)
    except ValueError as error:
        raise ReadinessError(f"{label} must be an ISO 8601 timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ReadinessError(f"{label} must include a timezone")
    parsed_utc = parsed.astimezone(dt.UTC)
    if parsed_utc > dt.datetime.now(dt.UTC):
        raise ReadinessError(f"{label} must not be in the future")
    return parsed_utc


def _require_timestamp(value: dict[str, Any], key: str, label: str) -> tuple[str, dt.datetime]:
    raw = _require_string(value, key, label)
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label}.{key} must not contain control characters")
    return raw, _parse_timestamp(raw, f"{label}.{key}")


def _validate_https_source_url(raw: str, label: str) -> str:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in raw):
        raise ReadinessError(f"{label} must not contain control characters")
    _reject_url_percent_encoding_smuggling(raw, label)
    if any(ch.isspace() for ch in raw):
        raise ReadinessError(f"{label} must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(raw)
        hostname = parsed.hostname
    except ValueError as error:
        raise ReadinessError(f"{label} is not a valid URL: {error}") from error
    if parsed.scheme != "https":
        raise ReadinessError(f"{label} must use HTTPS URL")
    try:
        port = parsed.port
    except ValueError as error:
        raise ReadinessError(f"{label} has invalid port: {error}") from error
    if port == 443:
        raise ReadinessError(f"{label} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise ReadinessError(f"{label} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise ReadinessError(f"{label} must not contain credentials")
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise ReadinessError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise ReadinessError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise ReadinessError(f"{label} host must not end with a dot")
    _validate_host_labels(raw_host, label)
    if parsed.params or parsed.query or parsed.fragment:
        raise ReadinessError(f"{label} must not contain params, query, or fragment")
    _validate_url_path(parsed, label)
    hostname = hostname.strip().lower()
    if hostname == "localhost" or hostname.endswith(".localhost"):
        raise ReadinessError(f"{label} must not use localhost")
    try:
        address = ipaddress.ip_address(hostname)
    except ValueError:
        return raw
    if not address.is_global:
        raise ReadinessError(f"{label} must not use local, private, or reserved IP addresses")
    return raw


def _raw_url_host(parsed: urllib.parse.ParseResult) -> str:
    netloc = parsed.netloc.rsplit("@", 1)[-1]
    if netloc.startswith("["):
        bracket = netloc.find("]")
        if bracket != -1:
            return netloc[1:bracket]
    return netloc.rsplit(":", 1)[0]


def _reject_url_percent_encoding_smuggling(url: str, label: str) -> None:
    index = 0
    while True:
        index = url.find("%", index)
        if index == -1:
            return
        token = url[index + 1 : index + 3]
        if len(token) != 2 or any(ch not in "0123456789abcdefABCDEF" for ch in token):
            raise ReadinessError(f"{label} must not contain malformed percent escapes")
        byte = int(token, 16)
        if byte <= 0x20 or byte == 0x7F:
            raise ReadinessError(
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
        raise ReadinessError(f"{label} host must be a valid IP address")
    labels = raw_host.split(".")
    if any(not part for part in labels):
        raise ReadinessError(f"{label} host must not contain empty labels")
    if all(part.isdigit() for part in labels):
        raise ReadinessError(f"{label} numeric host labels must be a valid IP address")
    for part in labels:
        if len(part) > 63:
            raise ReadinessError(f"{label} host labels must be at most 63 characters")
        if part.startswith("-") or part.endswith("-"):
            raise ReadinessError(f"{label} host labels must not start or end with hyphen")
        if not all(("a" <= ch <= "z") or ch.isdigit() or ch == "-" for ch in part):
            raise ReadinessError(
                f"{label} host labels must use lowercase ASCII letters, digits, or hyphens"
            )


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if "\\" in path:
        raise ReadinessError(f"{label} path must use forward slashes")
    if ";" in path:
        raise ReadinessError(f"{label} path must not contain semicolon parameters")
    segments = path.split("/")
    checked_segments = segments[1:] if path.startswith("/") else segments
    if any(segment == "" for segment in checked_segments[:-1]):
        raise ReadinessError(f"{label} path must not contain empty segments")
    if any(segment in {".", ".."} for segment in segments):
        raise ReadinessError(f"{label} path must not contain dot segments")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise ReadinessError(f"{label} path must not contain encoded dot or separator characters")
    if "%25" in lowered:
        raise ReadinessError(f"{label} path must not contain encoded percent characters")


def _require_summary_digest(summary: dict[str, Any], label: str) -> str:
    expected = summary.get(SUMMARY_DIGEST_FIELD)
    if not _is_lower_sha256(expected):
        raise ReadinessError(f"{label} has missing or non-canonical {SUMMARY_DIGEST_FIELD}")
    body = dict(summary)
    body.pop(SUMMARY_DIGEST_FIELD)
    actual = sha256_hex(_canonical_json_bytes(body))
    if actual != expected:
        raise ReadinessError(
            f"{label} {SUMMARY_DIGEST_FIELD} mismatch: expected {expected}, got {actual}"
        )
    return expected


def _blocker(blockers: list[dict[str, Any]], code: str, message: str, path: Path) -> None:
    blockers.append({"code": code, "message": message, "path": str(path)})


def _block_if_stale(
    timestamp: dt.datetime,
    *,
    max_age_days: int,
    code: str,
    label: str,
    path: Path,
    blockers: list[dict[str, Any]],
) -> None:
    cutoff = dt.datetime.now(dt.UTC) - dt.timedelta(days=max_age_days)
    if timestamp < cutoff:
        _blocker(
            blockers,
            code,
            f"{label} is older than the {max_age_days}-day freshness budget",
            path,
        )


def _block_duplicate_strings(
    values: list[str],
    *,
    label: str,
    code: str,
    message: str,
    path: Path,
    blockers: list[dict[str, Any]],
) -> None:
    seen: dict[str, int] = {}
    for offset, value in enumerate(values):
        if value in seen:
            _blocker(
                blockers,
                code,
                f"{message}: {label}[{offset}] duplicates {label}[{seen[value]}]",
                path,
            )
        else:
            seen[value] = offset


def _verify_xsd_summary_entries(
    summary: dict[str, Any],
    path: Path,
    *,
    verified_schemas: int,
    verified_fixtures: int,
    schema_backed_fixtures: int,
    schema_validated_fixtures: int,
    missing_schema_fixtures: list[Any],
    schema_only_entries: list[Any],
    blockers: list[dict[str, Any]],
) -> None:
    schemas_raw = _require_list(summary.get("schemas"), f"{path}.schemas")
    fixtures_raw = _require_list(summary.get("fixtures"), f"{path}.fixtures")
    if len(schemas_raw) != verified_schemas:
        _blocker(
            blockers,
            "xsd.schema_count_mismatch",
            "XSD summary verified_schemas does not match schemas[] length",
            path,
        )
    if len(fixtures_raw) != verified_fixtures:
        _blocker(
            blockers,
            "xsd.fixture_count_mismatch",
            "XSD summary verified_fixtures does not match fixtures[] length",
            path,
        )

    schema_paths: list[str] = []
    declared_schema_only_paths: list[str] = []
    declared_schema_only_entries: list[tuple[str, str, str]] = []
    schema_ids: list[str] = []
    schema_digests: list[str] = []
    schema_source_refs: list[str] = []
    schema_sources: list[dict[str, str]] = []
    for offset, schema_raw in enumerate(schemas_raw):
        label = f"{path}.schemas[{offset}]"
        schema = _require_object(schema_raw, label)
        _reject_unknown_keys(schema, XSD_SCHEMA_KEYS, label)
        message_def_id = _require_message_def_id(schema, "message_def_id", label)
        schema_path = _validate_schema_source_path(
            _require_string(schema, "path", label),
            f"{label}.path",
        )
        schema_paths.append(schema_path)
        if Path(schema_path).name != f"{message_def_id}.xsd":
            _blocker(
                blockers,
                "xsd.schema_path_mismatch",
                f"{label}.path filename does not match message_def_id",
                path,
            )
        schema_ids.append(message_def_id)
        _require_string(schema, "payload_root", label)
        _require_string(schema, "target_namespace", label)
        schema_sha256 = _require_sha256(schema, "sha256", label)
        schema_digests.append(schema_sha256)
        schema_only = _require_bool(schema, "schema_only", label)
        schema_only_reason = _validate_reviewed_gap_reason(
            schema.get("schema_only_reason"),
            f"{label}.schema_only_reason",
        )
        if schema_only:
            declared_schema_only_paths.append(schema_path)
            if schema_only_reason is None:
                _blocker(
                    blockers,
                    "xsd.schema_only_reason_absent",
                    f"{label} is marked schema-only without a reviewed reason",
                    path,
                )
            else:
                declared_schema_only_entries.append(
                    (schema_path, message_def_id, schema_only_reason)
                )
        elif schema_only_reason is not None:
            _blocker(
                blockers,
                "xsd.schema_only_reason_mismatch",
                f"{label} is not schema-only but still records a schema-only reason",
                path,
            )
        source = _verify_schema_source_summary(
            schema.get("source"),
            f"{label}.source",
            message_def_id=message_def_id,
            schema_sha256=schema_sha256,
            blockers=blockers,
            path=path,
        )
        schema_source_refs.append(
            f"{source['repository']}@{source['commit']}:{source['path']}"
        )
        schema_sources.append(source)
    _block_duplicate_strings(
        schema_paths,
        label=f"{path}.schemas.path",
        code="xsd.schema_path_duplicate",
        message="XSD summary repeats a schema path",
        path=path,
        blockers=blockers,
    )
    _block_duplicate_strings(
        schema_ids,
        label=f"{path}.schemas.message_def_id",
        code="xsd.schema_id_duplicate",
        message="XSD summary repeats a schema message_def_id",
        path=path,
        blockers=blockers,
    )
    _block_duplicate_strings(
        schema_digests,
        label=f"{path}.schemas.sha256",
        code="xsd.schema_digest_duplicate",
        message="XSD summary repeats a schema digest",
        path=path,
        blockers=blockers,
    )
    _block_duplicate_strings(
        schema_source_refs,
        label=f"{path}.schemas.source",
        code="xsd.schema_source_duplicate",
        message="XSD summary repeats a schema source reference",
        path=path,
        blockers=blockers,
    )

    fixture_paths: list[str] = []
    fixture_digests: list[str] = []
    backed_schema_paths: set[str] = set()
    computed_missing_schema_entries: list[tuple[str, str, str]] = []
    computed_schema_backed = 0
    computed_schema_validated = 0
    computed_missing_schema = 0
    schema_path_set = set(schema_paths)
    for offset, fixture_raw in enumerate(fixtures_raw):
        label = f"{path}.fixtures[{offset}]"
        fixture = _require_object(fixture_raw, label)
        _reject_unknown_keys(fixture, XSD_FIXTURE_KEYS, label)
        fixture_path = _validate_fixture_summary_path(
            _require_string(fixture, "path", label),
            f"{label}.path",
        )
        fixture_paths.append(fixture_path)
        fixture_message_def_id = _require_message_def_id(fixture, "message_def_id", label)
        _require_string(fixture, "payload_root", label)
        fixture_digests.append(_require_sha256(fixture, "sha256", label))
        schema_backed = _require_bool(fixture, "schema_backed", label)
        schema_validated = _require_bool(fixture, "schema_validated", label)
        schema_rel = fixture.get("schema")
        missing_reason = _validate_reviewed_gap_reason(
            fixture.get("missing_schema_reason"),
            f"{label}.missing_schema_reason",
        )
        if schema_backed:
            computed_schema_backed += 1
            if schema_validated:
                computed_schema_validated += 1
            if missing_reason is not None:
                _blocker(
                    blockers,
                    "xsd.fixture_missing_schema_reason_mismatch",
                    f"{label} is schema-backed but still records a missing-schema reason",
                    path,
                )
            if not isinstance(schema_rel, str) or not schema_rel.strip():
                _blocker(
                    blockers,
                    "xsd.fixture_schema_reference_missing",
                    f"{label} is schema-backed but has no schema reference",
                    path,
                )
            else:
                schema_rel = _validate_schema_source_path(schema_rel, f"{label}.schema")
                if schema_rel not in schema_path_set:
                    _blocker(
                        blockers,
                        "xsd.fixture_schema_reference_mismatch",
                        f"{label}.schema references an unknown schema path",
                        path,
                    )
                else:
                    backed_schema_paths.add(schema_rel)
        else:
            computed_missing_schema += 1
            if schema_validated:
                _blocker(
                    blockers,
                    "xsd.fixture_unbacked_schema_validated",
                    f"{label} is marked schema_validated without a schema reference",
                    path,
                )
            if schema_rel is not None:
                _blocker(
                    blockers,
                    "xsd.fixture_schema_backing_mismatch",
                    f"{label} is marked unbacked but still records a schema reference",
                    path,
                )
            if missing_reason is None:
                _blocker(
                    blockers,
                    "xsd.fixture_missing_schema_reason_absent",
                    f"{label} is not schema-backed but has no reviewed missing-schema reason",
                    path,
                )
            else:
                computed_missing_schema_entries.append(
                    (fixture_path, fixture_message_def_id, missing_reason)
                )
    _block_duplicate_strings(
        fixture_paths,
        label=f"{path}.fixtures.path",
        code="xsd.fixture_path_duplicate",
        message="XSD summary repeats a fixture path",
        path=path,
        blockers=blockers,
    )
    _block_duplicate_strings(
        fixture_digests,
        label=f"{path}.fixtures.sha256",
        code="xsd.fixture_digest_duplicate",
        message="XSD summary repeats a fixture digest",
        path=path,
        blockers=blockers,
    )
    if computed_schema_backed != schema_backed_fixtures:
        _blocker(
            blockers,
            "xsd.schema_backed_count_mismatch",
            "XSD summary schema_backed_fixtures does not match fixtures[]",
            path,
        )
    if computed_schema_validated != schema_validated_fixtures:
        _blocker(
            blockers,
            "xsd.schema_validated_count_mismatch",
            "XSD summary schema_validated_fixtures does not match fixtures[]",
            path,
        )
    if computed_schema_validated != computed_schema_backed:
        _blocker(
            blockers,
            "xsd.schema_backed_fixtures_not_validated",
            "not all schema-backed XML fixtures were validated against their XSDs",
            path,
        )
    if computed_missing_schema != len(missing_schema_fixtures):
        _blocker(
            blockers,
            "xsd.missing_schema_fixture_count_mismatch",
            "XSD summary missing_schema_fixtures does not match fixtures[]",
            path,
        )
    actual_missing_schema_entries: list[tuple[str, str, str]] = []
    for offset, raw_missing in enumerate(missing_schema_fixtures):
        label = f"{path}.missing_schema_fixtures[{offset}]"
        missing = _require_object(raw_missing, label)
        _reject_unknown_keys(missing, XSD_GAP_ENTRY_KEYS, label)
        actual_missing_schema_entries.append(_xsd_gap_entry_key(missing, label))
    if sorted(actual_missing_schema_entries) != sorted(computed_missing_schema_entries):
        _blocker(
            blockers,
            "xsd.missing_schema_fixture_entries_mismatch",
            "XSD summary missing_schema_fixtures does not match fixtures[] entries",
            path,
        )
    computed_schema_only_paths = schema_path_set - backed_schema_paths
    if set(declared_schema_only_paths) != computed_schema_only_paths:
        _blocker(
            blockers,
            "xsd.schema_only_flag_mismatch",
            "XSD summary schemas[].schema_only does not match schemas[]/fixtures[]",
            path,
        )
    computed_schema_only = len(computed_schema_only_paths)
    if computed_schema_only != len(schema_only_entries):
        _blocker(
            blockers,
            "xsd.schema_only_count_mismatch",
            "XSD summary schema_only_entries does not match schemas[]/fixtures[]",
            path,
        )
    actual_schema_only_entries: list[tuple[str, str, str]] = []
    for offset, raw_schema_only in enumerate(schema_only_entries):
        label = f"{path}.schema_only_entries[{offset}]"
        schema_only = _require_object(raw_schema_only, label)
        _reject_unknown_keys(schema_only, XSD_GAP_ENTRY_KEYS, label)
        actual_schema_only_entries.append(_xsd_gap_entry_key(schema_only, label))
    if sorted(actual_schema_only_entries) != sorted(declared_schema_only_entries):
        _blocker(
            blockers,
            "xsd.schema_only_entries_mismatch",
            "XSD summary schema_only_entries does not match schemas[] entries",
            path,
        )
    summary["_validated_schema_sources"] = schema_sources


def _xsd_gap_entry_key(entry: dict[str, Any], label: str) -> tuple[str, str, str]:
    return (
        _require_string(entry, "path", label),
        _require_message_def_id(entry, "message_def_id", label),
        _require_string(entry, "reason", label),
    )


def _profile_version_key(entry: dict[str, Any], label: str) -> tuple[str, str, str, str]:
    profile_id = _require_profile_id(entry, "profile_id", label)
    message_type = _require_message_type(entry, "message_type", label)
    direction = _require_profile_direction(entry, "direction", label)
    message_def_id = _require_message_def_id(entry, "message_def_id", label)
    if not message_def_id.startswith(message_type + "."):
        raise ReadinessError(
            f"{label}.message_def_id must match message_type {message_type!r}"
        )
    return profile_id, message_type, direction, message_def_id


def _verify_xsd_profile_catalog_entries(
    summary: dict[str, Any],
    path: Path,
    *,
    profile_checked_versions: int,
    profile_schema_backed_versions: int,
    missing_profile_schema_versions: list[Any],
    blockers: list[dict[str, Any]],
) -> dict[str, str] | None:
    profile_catalog_raw = summary.get("profile_catalog")
    if profile_catalog_raw is None:
        if (
            profile_checked_versions
            or profile_schema_backed_versions
            or missing_profile_schema_versions
        ):
            _blocker(
                blockers,
                "xsd.profile_catalog_count_mismatch",
                "XSD summary records profile counts without a profile_catalog section",
                path,
            )
        return None

    profile_catalog = _require_object(profile_catalog_raw, f"{path}.profile_catalog")
    _reject_unknown_keys(profile_catalog, XSD_PROFILE_CATALOG_KEYS, f"{path}.profile_catalog")
    profile_catalog_path = _require_string(
        profile_catalog,
        "path",
        f"{path}.profile_catalog",
    )
    _reject_path_smuggling(profile_catalog_path, f"{path}.profile_catalog.path")
    profile_catalog_sha256 = _require_sha256(
        profile_catalog,
        "sha256",
        f"{path}.profile_catalog",
    )
    profile_catalog_json_sha256 = _require_sha256(
        profile_catalog,
        "catalog_json_sha256",
        f"{path}.profile_catalog",
    )
    catalog_profiles = _require_positive_int(
        profile_catalog,
        "profiles",
        f"{path}.profile_catalog",
    )
    catalog_checked_versions = _require_nonnegative_int(
        profile_catalog,
        "checked_versions",
        f"{path}.profile_catalog",
    )
    catalog_schema_backed_versions = _require_nonnegative_int(
        profile_catalog,
        "schema_backed_versions",
        f"{path}.profile_catalog",
    )
    if catalog_checked_versions != profile_checked_versions:
        _blocker(
            blockers,
            "xsd.profile_catalog_checked_count_mismatch",
            "XSD profile_catalog.checked_versions does not match top-level count",
            path,
        )
    if catalog_schema_backed_versions != profile_schema_backed_versions:
        _blocker(
            blockers,
            "xsd.profile_catalog_schema_backed_count_mismatch",
            "XSD profile_catalog.schema_backed_versions does not match top-level count",
            path,
        )
    skipped_raw = _require_list(
        profile_catalog.get("skipped_family_versions"),
        f"{path}.profile_catalog.skipped_family_versions",
    )
    seen_skipped: dict[tuple[str, str, str, str], int] = {}
    for offset, raw_skipped in enumerate(skipped_raw):
        label = f"{path}.profile_catalog.skipped_family_versions[{offset}]"
        skipped = _require_object(raw_skipped, label)
        _reject_unknown_keys(skipped, XSD_PROFILE_SKIPPED_VERSION_KEYS, label)
        key = (
            _require_profile_id(skipped, "profile_id", label),
            _require_message_type(skipped, "message_type", label),
            _require_profile_direction(skipped, "direction", label),
            _require_string(skipped, "version", label),
        )
        if MESSAGE_DEF_ID_RE.fullmatch(key[3]) is not None:
            _blocker(
                blockers,
                "xsd.profile_catalog_skipped_concrete_version",
                f"{label}.version is concrete and should not be skipped",
                path,
            )
        elif key[3] != key[1]:
            _blocker(
                blockers,
                "xsd.profile_catalog_skipped_family_mismatch",
                f"{label}.version must equal message_type {key[1]!r}",
                path,
            )
        if key in seen_skipped:
            _blocker(
                blockers,
                "xsd.profile_catalog_skipped_duplicate",
                (
                    f"{label} duplicates "
                    f"{path}.profile_catalog.skipped_family_versions[{seen_skipped[key]}]"
                ),
                path,
            )
        else:
            seen_skipped[key] = offset
    versions_raw = _require_list(
        profile_catalog.get("versions"),
        f"{path}.profile_catalog.versions",
    )
    if len(versions_raw) != profile_checked_versions:
        _blocker(
            blockers,
            "xsd.profile_version_count_mismatch",
            "XSD summary profile_checked_versions does not match profile_catalog.versions[]",
            path,
        )
    computed_schema_backed = 0
    computed_missing: list[tuple[str, str, str, str]] = []
    seen_versions: dict[tuple[str, str, str, str], int] = {}
    for offset, raw_version in enumerate(versions_raw):
        label = f"{path}.profile_catalog.versions[{offset}]"
        version = _require_object(raw_version, label)
        _reject_unknown_keys(version, XSD_PROFILE_VERSION_KEYS, label)
        key = _profile_version_key(version, label)
        if key in seen_versions:
            _blocker(
                blockers,
                "xsd.profile_version_duplicate",
                (
                    f"{label} duplicates "
                    f"{path}.profile_catalog.versions[{seen_versions[key]}]"
                ),
                path,
            )
        else:
            seen_versions[key] = offset
        schema_backed = _require_bool(version, "schema_backed", label)
        if schema_backed:
            computed_schema_backed += 1
        else:
            computed_missing.append(key)
    if computed_schema_backed != profile_schema_backed_versions:
        _blocker(
            blockers,
            "xsd.profile_schema_backed_count_mismatch",
            "XSD summary profile_schema_backed_versions does not match profile catalog",
            path,
        )

    catalog_missing: list[tuple[str, str, str, str]] = []
    catalog_missing_raw = _require_list(
        profile_catalog.get("missing_schema_versions"),
        f"{path}.profile_catalog.missing_schema_versions",
    )
    for offset, raw_missing in enumerate(catalog_missing_raw):
        label = f"{path}.profile_catalog.missing_schema_versions[{offset}]"
        missing = _require_object(raw_missing, label)
        _reject_unknown_keys(missing, XSD_PROFILE_MISSING_VERSION_KEYS, label)
        catalog_missing.append(_profile_version_key(missing, label))
    if sorted(catalog_missing) != sorted(computed_missing):
        _blocker(
            blockers,
            "xsd.profile_catalog_missing_schema_versions_mismatch",
            "XSD summary profile_catalog.missing_schema_versions does not match profile catalog",
            path,
        )

    actual_missing: list[tuple[str, str, str, str]] = []
    for offset, raw_missing in enumerate(missing_profile_schema_versions):
        label = f"{path}.missing_profile_schema_versions[{offset}]"
        missing = _require_object(raw_missing, label)
        _reject_unknown_keys(missing, XSD_PROFILE_MISSING_VERSION_KEYS, label)
        actual_missing.append(_profile_version_key(missing, label))
    if sorted(actual_missing) != sorted(computed_missing):
        _blocker(
            blockers,
            "xsd.missing_profile_schema_versions_mismatch",
            "XSD summary missing_profile_schema_versions does not match profile catalog",
            path,
        )
    return {
        "path": profile_catalog_path,
        "sha256": profile_catalog_sha256,
        "catalog_json_sha256": profile_catalog_json_sha256,
        "profiles": catalog_profiles,
    }


def _verify_receipt_summary(
    receipt_obj: dict[str, Any],
    label: str,
    path: Path,
    blockers: list[dict[str, Any]],
    *,
    missing_kinds_code: str,
    allow_failed_code: str,
    allow_insecure_code: str,
    allow_legacy_code: str,
    source_files_code: str,
    count_mismatch_code: str,
    digest_missing_code: str,
    duplicate_path_code: str,
    duplicate_digest_code: str,
    kind_entry_mismatch_code: str,
) -> dict[str, Any]:
    digest = _require_summary_digest(receipt_obj, label)
    _reject_unknown_keys(receipt_obj, RECEIPT_SUMMARY_KEYS, label)
    verified_receipts = _require_positive_int(
        receipt_obj,
        "verified_receipts",
        label,
    )
    receipt_kind_raw = _require_list(
        receipt_obj.get("receipt_kind"),
        f"{label}.receipt_kind",
    )
    seen_receipt_kinds: dict[str, int] = {}
    for offset, item in enumerate(receipt_kind_raw):
        if not isinstance(item, str) or not item.strip():
            raise ReadinessError(f"{label}.receipt_kind must contain strings")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise ReadinessError(
                f"{label}.receipt_kind[{offset}] must not contain control characters"
            )
        if item != item.strip():
            raise ReadinessError(
                f"{label}.receipt_kind[{offset}] must not have surrounding whitespace"
            )
        if item in seen_receipt_kinds:
            _blocker(
                blockers,
                kind_entry_mismatch_code,
                (
                    f"{label}.receipt_kind[{offset}] duplicates "
                    f"{label}.receipt_kind[{seen_receipt_kinds[item]}]"
                ),
                path,
            )
        else:
            seen_receipt_kinds[item] = offset
    receipt_kind_set = set(receipt_kind_raw)
    missing = sorted(REQUIRED_RECEIPT_KINDS - receipt_kind_set)
    if missing:
        _blocker(
            blockers,
            missing_kinds_code,
            "receipt verification is missing kinds: " + ", ".join(missing),
            path,
        )
    unsupported = sorted(receipt_kind_set - REQUIRED_RECEIPT_KINDS)
    if unsupported:
        _blocker(
            blockers,
            kind_entry_mismatch_code,
            "receipt verification contains unsupported kinds: " + ", ".join(unsupported),
            path,
        )
    allow_failed = _require_bool(receipt_obj, "allow_failed", label)
    allow_insecure_http = _require_bool(receipt_obj, "allow_insecure_http", label)
    allow_legacy_colr007 = _require_bool(receipt_obj, "allow_legacy_colr007", label)
    require_source_files = _require_bool(receipt_obj, "require_source_files", label)
    if allow_failed:
        _blocker(
            blockers,
            allow_failed_code,
            "receipt verifier evidence allowed failed receipts",
            path,
        )
    if allow_insecure_http:
        _blocker(
            blockers,
            allow_insecure_code,
            "receipt verifier evidence allowed insecure HTTP endpoints",
            path,
        )
    if allow_legacy_colr007:
        _blocker(
            blockers,
            allow_legacy_code,
            "receipt verifier evidence allowed legacy colr.007 rail receipts",
            path,
        )
    if not require_source_files:
        _blocker(
            blockers,
            source_files_code,
            "receipt verifier evidence did not require source files",
            path,
        )

    receipts_raw = _require_list(receipt_obj.get("receipts"), f"{label}.receipts")
    if len(receipts_raw) != verified_receipts:
        _blocker(
            blockers,
            count_mismatch_code,
            "receipt verification count does not match receipts[] entries",
            path,
        )
    receipts: list[dict[str, Any]] = []
    receipt_entry_kinds: set[str] = set()
    seen_receipt_paths: dict[str, int] = {}
    seen_receipt_digests: dict[str, int] = {}
    for offset, receipt_raw in enumerate(receipts_raw):
        entry_label = f"{label}.receipts[{offset}]"
        receipt = _require_object(receipt_raw, entry_label)
        _reject_unknown_keys(receipt, RECEIPT_ENTRY_KEYS, entry_label)
        receipt_path = _validate_receipt_path(
            _require_string(receipt, "path", entry_label),
            f"{entry_label}.path",
        )
        if receipt_path in seen_receipt_paths:
            _blocker(
                blockers,
                duplicate_path_code,
                (
                    f"{entry_label}.path duplicates "
                    f"{label}.receipts[{seen_receipt_paths[receipt_path]}].path"
                ),
                path,
            )
        else:
            seen_receipt_paths[receipt_path] = offset
        receipt_kind = _require_string(receipt, "receipt_kind", entry_label)
        if receipt_kind not in REQUIRED_RECEIPT_KINDS:
            _blocker(
                blockers,
                kind_entry_mismatch_code,
                f"{entry_label}.receipt_kind is unsupported: {receipt_kind!r}",
                path,
            )
        receipt_sha256 = receipt.get("receipt_sha256")
        if not _is_lower_sha256(receipt_sha256):
            _blocker(
                blockers,
                digest_missing_code,
                f"{entry_label}.receipt_sha256 is missing or non-canonical",
                path,
            )
        elif receipt_sha256 in seen_receipt_digests:
            _blocker(
                blockers,
                duplicate_digest_code,
                (
                    f"{entry_label}.receipt_sha256 duplicates "
                    f"{label}.receipts[{seen_receipt_digests[receipt_sha256]}].receipt_sha256"
                ),
                path,
            )
        else:
            seen_receipt_digests[receipt_sha256] = offset
        receipts.append(dict(receipt))
        receipt_entry_kinds.add(receipt_kind)
    if receipt_kind_set != receipt_entry_kinds:
        _blocker(
            blockers,
            kind_entry_mismatch_code,
            "receipt_kind does not match receipts[].receipt_kind",
            path,
        )

    return {
        "verified_receipts": verified_receipts,
        "receipt_kind": sorted(receipt_kind_set),
        "allow_failed": allow_failed,
        "allow_insecure_http": allow_insecure_http,
        "allow_legacy_colr007": allow_legacy_colr007,
        "require_source_files": require_source_files,
        "receipts": receipts,
        "summary_sha256": digest,
    }


def verify_xsd_summary(
    path: Path,
    *,
    allow_reviewed_xsd_gaps: bool,
    max_age_days: int,
    blockers: list[dict[str, Any]],
    warnings: list[dict[str, Any]],
) -> dict[str, Any]:
    """Verify one XSD preflight summary and append production blockers."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _reject_unknown_keys(summary, XSD_SUMMARY_KEYS, str(path))
    verified_at, verified_at_dt = _require_timestamp(summary, "verified_at", str(path))
    _block_if_stale(
        verified_at_dt,
        max_age_days=max_age_days,
        code="xsd.summary_stale",
        label="XSD summary verified_at",
        path=path,
        blockers=blockers,
    )
    manifest_sha256 = _require_sha256(summary, "manifest_sha256", str(path))
    verified_schemas = _require_positive_int(summary, "verified_schemas", str(path))
    verified_fixtures = _require_positive_int(summary, "verified_fixtures", str(path))
    schema_backed_fixtures = summary.get("schema_backed_fixtures")
    if (
        isinstance(schema_backed_fixtures, bool)
        or not isinstance(schema_backed_fixtures, int)
        or schema_backed_fixtures < 0
    ):
        raise ReadinessError(f"{path}.schema_backed_fixtures must be a non-negative integer")
    schema_validated_fixtures = summary.get("schema_validated_fixtures")
    if (
        isinstance(schema_validated_fixtures, bool)
        or not isinstance(schema_validated_fixtures, int)
        or schema_validated_fixtures < 0
    ):
        raise ReadinessError(
            f"{path}.schema_validated_fixtures must be a non-negative integer"
        )
    profile_checked_versions = _require_nonnegative_int(
        summary,
        "profile_checked_versions",
        str(path),
    )
    profile_schema_backed_versions = _require_nonnegative_int(
        summary,
        "profile_schema_backed_versions",
        str(path),
    )
    missing_schema_fixtures = _require_list(
        summary.get("missing_schema_fixtures"),
        f"{path}.missing_schema_fixtures",
    )
    schema_only_entries = _require_list(
        summary.get("schema_only_entries"),
        f"{path}.schema_only_entries",
    )
    missing_profile_schema_versions = _require_list(
        summary.get("missing_profile_schema_versions"),
        f"{path}.missing_profile_schema_versions",
    )
    strict = _require_object(summary.get("strict"), f"{path}.strict")
    _reject_unknown_keys(strict, XSD_STRICT_KEYS, f"{path}.strict")
    require_schema_backed = _require_bool(
        strict,
        "require_schema_backed_fixtures",
        f"{path}.strict",
    )
    require_fixture_for_schema = _require_bool(
        strict,
        "require_fixture_for_schema",
        f"{path}.strict",
    )
    require_profile_schema_backed = _require_bool(
        strict,
        "require_profile_schema_backed_versions",
        f"{path}.strict",
    )
    validate_xml_schema = _require_bool(
        strict,
        "validate_xml_schema",
        f"{path}.strict",
    )
    if schema_backed_fixtures > verified_fixtures:
        raise ReadinessError(f"{path}.schema_backed_fixtures exceeds verified_fixtures")
    if schema_validated_fixtures > schema_backed_fixtures:
        raise ReadinessError(
            f"{path}.schema_validated_fixtures exceeds schema_backed_fixtures"
        )
    if profile_schema_backed_versions > profile_checked_versions:
        raise ReadinessError(
            f"{path}.profile_schema_backed_versions exceeds profile_checked_versions"
        )
    _verify_xsd_summary_entries(
        summary,
        path,
        verified_schemas=verified_schemas,
        verified_fixtures=verified_fixtures,
        schema_backed_fixtures=schema_backed_fixtures,
        schema_validated_fixtures=schema_validated_fixtures,
        missing_schema_fixtures=missing_schema_fixtures,
        schema_only_entries=schema_only_entries,
        blockers=blockers,
    )
    profile_catalog_summary = _verify_xsd_profile_catalog_entries(
        summary,
        path,
        profile_checked_versions=profile_checked_versions,
        profile_schema_backed_versions=profile_schema_backed_versions,
        missing_profile_schema_versions=missing_profile_schema_versions,
        blockers=blockers,
    )

    if not require_schema_backed:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.strict_schema_backed_not_proven",
                "message": "XSD summary was not produced with --require-schema-backed-fixtures",
                "path": str(path),
            }
        )
    if not require_fixture_for_schema:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.strict_fixture_for_schema_not_proven",
                "message": "XSD summary was not produced with --require-fixture-for-schema",
                "path": str(path),
            }
        )
    if not require_profile_schema_backed:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.profile_schema_backed_not_proven",
                "message": (
                    "XSD summary was not produced with "
                    "--require-profile-schema-backed-versions"
                ),
                "path": str(path),
            }
        )
    if not validate_xml_schema:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.xml_schema_validation_not_proven",
                "message": "XSD summary was not produced with --validate-xml-schema",
                "path": str(path),
            }
        )
    if missing_schema_fixtures:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.missing_schema_fixtures",
                "message": f"{len(missing_schema_fixtures)} XML fixtures are not schema-backed",
                "path": str(path),
                "entries": missing_schema_fixtures,
            }
        )
    if schema_only_entries:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.schema_only_entries",
                "message": f"{len(schema_only_entries)} XSDs have no standalone XML fixture",
                "path": str(path),
                "entries": schema_only_entries,
            }
        )
    if require_profile_schema_backed and profile_checked_versions == 0:
        _blocker(
            blockers,
            "xsd.profile_catalog_empty",
            "XSD summary did not verify any profile catalog message versions",
            path,
        )
    if missing_profile_schema_versions:
        target = warnings if allow_reviewed_xsd_gaps else blockers
        target.append(
            {
                "code": "xsd.missing_profile_schema_versions",
                "message": (
                    f"{len(missing_profile_schema_versions)} advertised profile "
                    "message versions are not schema-backed"
                ),
                "path": str(path),
                "entries": missing_profile_schema_versions,
            }
        )
    return {
        "path": str(path),
        "verified_at": verified_at,
        "manifest_sha256": manifest_sha256,
        "verified_schemas": verified_schemas,
        "verified_fixtures": verified_fixtures,
        "schema_backed_fixtures": schema_backed_fixtures,
        "schema_validated_fixtures": schema_validated_fixtures,
        "profile_checked_versions": profile_checked_versions,
        "profile_schema_backed_versions": profile_schema_backed_versions,
        "schema_sources": summary.get("_validated_schema_sources", []),
        "missing_schema_fixture_count": len(missing_schema_fixtures),
        "schema_only_count": len(schema_only_entries),
        "missing_profile_schema_version_count": len(missing_profile_schema_versions),
        "profile_catalog": profile_catalog_summary,
        "strict": {
            "require_schema_backed_fixtures": require_schema_backed,
            "require_fixture_for_schema": require_fixture_for_schema,
            "require_profile_schema_backed_versions": require_profile_schema_backed,
            "validate_xml_schema": validate_xml_schema,
        },
        "summary_sha256": digest,
    }


def _verify_policy(
    summary: dict[str, Any],
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    policy = _require_object(summary.get("policy"), f"{path}.policy")
    _reject_unknown_keys(policy, EVIDENCE_POLICY_KEYS, f"{path}.policy")
    provider = _require_string(policy, "provider", f"{path}.policy")
    environment = _require_string(policy, "environment", f"{path}.policy")
    if provider != args.provider:
        _blocker(
            blockers,
            "evidence.policy_provider_mismatch",
            f"evidence policy provider is {provider!r}, expected {args.provider!r}",
            path,
        )
    if environment != args.environment:
        _blocker(
            blockers,
            "evidence.policy_environment_mismatch",
            f"evidence policy environment is {environment!r}, expected {args.environment!r}",
            path,
        )
    for flag in sorted(PRODUCTION_FALSE_POLICY_FLAGS):
        if _require_bool(policy, flag, f"{path}.policy"):
            if flag == "allow_canary_stage_receipts_only" and args.allow_canary_stage_receipts_only:
                continue
            _blocker(
                blockers,
                f"evidence.policy.{flag}",
                f"Evidence summary was produced with non-production policy {flag}=true",
                path,
            )
    freshness: dict[str, int] = {}
    for field in sorted(EVIDENCE_FRESHNESS_POLICY_FIELDS):
        value = _require_positive_int(policy, field, f"{path}.policy")
        freshness[field] = value
        if value > getattr(args, field):
            _blocker(
                blockers,
                f"evidence.policy.{field}_weaker_than_release",
                (
                    f"Evidence summary was produced with {field}={value}, "
                    f"which is weaker than release {field}={getattr(args, field)}"
                ),
                path,
            )
    return {"provider": provider, "environment": environment, **freshness}


def _verify_canary(
    canary: dict[str, Any],
    label: str,
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    _reject_unknown_keys(canary, COMPACT_CANARY_KEYS, label)
    canary_path = _validate_compact_summary_path(
        _require_string(canary, "path", label),
        f"{label}.path",
    )
    config_path = _validate_config_path(
        _require_string(canary, "config_path", label),
        f"{label}.config_path",
    )
    summary_sha256 = _require_sha256(canary, SUMMARY_DIGEST_FIELD, label)
    started_at_raw, started_at = _require_timestamp(canary, "started_at", label)
    finished_at_raw, finished_at = _require_timestamp(canary, "finished_at", label)
    if finished_at < started_at:
        raise ReadinessError(f"{label}.finished_at must not be before started_at")
    _block_if_stale(
        finished_at,
        max_age_days=args.max_canary_age_days,
        code="evidence.canary_stale",
        label="canary finished_at",
        path=path,
        blockers=blockers,
    )
    provider = _require_string(canary, "provider", label)
    environment = _require_string(canary, "environment", label)
    plan_only = _require_bool(canary, "plan_only", label)
    require_explicit_policy = _require_bool(canary, "require_explicit_policy", label)
    if args.provider is not None and provider != args.provider:
        _blocker(
            blockers,
            "evidence.provider_mismatch",
            f"canary provider is {provider!r}, expected {args.provider!r}",
            path,
        )
    if args.environment is not None and environment != args.environment:
        _blocker(
            blockers,
            "evidence.environment_mismatch",
            f"canary environment is {environment!r}, expected {args.environment!r}",
            path,
        )
    if plan_only:
        _blocker(blockers, "evidence.plan_only", "canary summary is plan-only", path)
    if not require_explicit_policy:
        _blocker(
            blockers,
            "evidence.canary_implicit_policy",
            "canary summary does not prove --require-explicit-policy",
            path,
        )
    stage_names_raw = _require_list(canary.get("stage_names"), f"{label}.stage_names")
    stage_names_clean: list[str] = []
    for offset, item in enumerate(stage_names_raw):
        if not isinstance(item, str) or not item.strip():
            raise ReadinessError(f"{label}.stage_names must contain non-empty strings")
        if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in item):
            raise ReadinessError(
                f"{label}.stage_names[{offset}] must not contain control characters"
            )
        if item != item.strip():
            raise ReadinessError(
                f"{label}.stage_names[{offset}] must not have surrounding whitespace"
            )
        stage_names_clean.append(item)
    stage_names_raw = stage_names_clean
    stage_names = set(stage_names_raw)
    if len(stage_names_raw) != len(stage_names):
        raise ReadinessError(f"{label}.stage_names must not contain duplicates")
    unsupported_stages = sorted(stage_names - REQUIRED_CANARY_STAGES)
    if unsupported_stages:
        raise ReadinessError(
            f"{label}.stage_names contains unsupported stages: "
            + ", ".join(unsupported_stages)
        )
    stage_windows_raw = _require_list(canary.get("stage_windows"), f"{label}.stage_windows")
    stage_windows: list[dict[str, str]] = []
    stage_window_names: list[str] = []
    previous_window_finished: dt.datetime | None = None
    for offset, raw_window in enumerate(stage_windows_raw):
        window_label = f"{label}.stage_windows[{offset}]"
        window = _require_object(raw_window, window_label)
        _reject_unknown_keys(window, COMPACT_STAGE_WINDOW_KEYS, window_label)
        stage_name = _require_string(window, "name", window_label)
        window_started_raw, window_started = _require_timestamp(
            window,
            "started_at",
            window_label,
        )
        window_finished_raw, window_finished = _require_timestamp(
            window,
            "finished_at",
            window_label,
        )
        if window_finished < window_started:
            raise ReadinessError(f"{window_label}.finished_at must not be before started_at")
        if window_started < started_at or window_finished > finished_at:
            raise ReadinessError(f"{window_label} timestamp window must be inside canary window")
        if (
            previous_window_finished is not None
            and window_started < previous_window_finished
        ):
            raise ReadinessError(
                f"{window_label}.started_at must not be before previous stage finished_at"
            )
        stage_windows.append(
            {
                "name": stage_name,
                "started_at": window_started_raw,
                "finished_at": window_finished_raw,
            }
        )
        stage_window_names.append(stage_name)
        previous_window_finished = window_finished
    if stage_window_names != stage_names_raw:
        raise ReadinessError(f"{label}.stage_windows must match stage_names")
    missing_stages = sorted(REQUIRED_CANARY_STAGES - stage_names)
    if missing_stages:
        _blocker(
            blockers,
            "evidence.missing_canary_stages",
            "canary summary is missing stages: " + ", ".join(missing_stages),
            path,
        )
    receipt_summary = _verify_receipt_summary(
        _require_object(canary.get("receipt_summary"), f"{label}.receipt_summary"),
        f"{label}.receipt_summary",
        path,
        blockers,
        missing_kinds_code="evidence.missing_receipt_kinds",
        allow_failed_code="evidence.receipts_allow_failed",
        allow_insecure_code="evidence.receipts_allow_insecure_http",
        allow_legacy_code="evidence.receipts_allow_legacy_colr007",
        source_files_code="evidence.receipts_source_files_not_required",
        count_mismatch_code="evidence.receipt_count_mismatch",
        digest_missing_code="evidence.receipt_digest_missing",
        duplicate_path_code="evidence.receipt_path_duplicate",
        duplicate_digest_code="evidence.receipt_digest_duplicate",
        kind_entry_mismatch_code="evidence.receipt_kind_entry_mismatch",
    )
    return {
        "path": canary_path,
        "config_path": config_path,
        "started_at": started_at_raw,
        "finished_at": finished_at_raw,
        "provider": provider,
        "environment": environment,
        "plan_only": plan_only,
        "require_explicit_policy": require_explicit_policy,
        "stage_names": list(stage_names_raw),
        "stage_windows": stage_windows,
        "verified_receipts": receipt_summary["verified_receipts"],
        "receipt_kind": receipt_summary["receipt_kind"],
        "receipt_summary": receipt_summary,
        "summary_sha256": summary_sha256,
    }


def _verify_trust_profile(
    profile: dict[str, Any],
    label: str,
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    _reject_unknown_keys(profile, TRUST_PROFILE_KEYS, label)
    profile_id = _require_string(profile, "profile_id", label)
    rail = _require_string(profile, "rail", label)
    environment = _require_string(profile, "environment", label)
    bundle_sha256 = _require_sha256(profile, "bundle_sha256", label)
    policy = _require_string(profile, "embedded_signature_policy", label)
    signature_pin_count = _require_nonnegative_int(
        profile,
        "signature_public_key_pin_count",
        label,
    )
    x509_pin_count = _require_nonnegative_int(
        profile,
        "x509_trust_anchor_pin_count",
        label,
    )
    trust_anchor_der = _require_compact_der_entries(
        profile,
        "x509_trust_anchor_der",
        label,
    )
    if len(trust_anchor_der) > x509_pin_count:
        raise ReadinessError(
            f"{label}.x509_trust_anchor_der length exceeds x509_trust_anchor_pin_count"
        )
    revoked_pin_count = _require_nonnegative_int(
        profile,
        "revoked_certificate_pin_count",
        label,
    )
    revoked_der = _require_compact_der_entries(
        profile,
        "revoked_certificate_der",
        label,
    )
    if len(revoked_der) > revoked_pin_count:
        raise ReadinessError(
            f"{label}.revoked_certificate_der length exceeds revoked_certificate_pin_count"
        )
    policy_oid_count = _require_nonnegative_int(
        profile,
        "x509_required_certificate_policy_oid_count",
        label,
    )
    crl_required = _require_bool(profile, "x509_require_crl_revocation_check", label)
    x509_crl_count = _require_nonnegative_int(profile, "x509_crl_count", label)
    x509_crl_der = _require_compact_der_entries(profile, "x509_crl_der", label)
    if len(x509_crl_der) != x509_crl_count:
        raise ReadinessError(f"{label}.x509_crl_der length does not match x509_crl_count")
    ocsp_required = _require_bool(profile, "x509_require_ocsp_revocation_check", label)
    x509_ocsp_response_count = _require_nonnegative_int(
        profile,
        "x509_ocsp_response_count",
        label,
    )
    x509_ocsp_response_der = _require_compact_der_entries(
        profile,
        "x509_ocsp_response_der",
        label,
    )
    if len(x509_ocsp_response_der) != x509_ocsp_response_count:
        raise ReadinessError(
            f"{label}.x509_ocsp_response_der length does not match x509_ocsp_response_count"
        )
    source = _require_object(profile.get("source"), f"{label}.source")
    _reject_unknown_keys(source, TRUST_SOURCE_KEYS, f"{label}.source")
    source_authority = _require_string(source, "authority", f"{label}.source")
    source_version = _require_string(source, "version", f"{label}.source")
    source_url = _validate_https_source_url(
        _require_string(source, "url", f"{label}.source"),
        f"{label}.source.url",
    )
    source_retrieved_at_raw, source_retrieved_at = _require_timestamp(
        source,
        "retrieved_at",
        f"{label}.source",
    )
    _block_if_stale(
        source_retrieved_at,
        max_age_days=args.max_trust_source_age_days,
        code="trust.source_stale",
        label="trust source retrieved_at",
        path=path,
        blockers=blockers,
    )
    if signature_pin_count + x509_pin_count <= 0:
        _blocker(
            blockers,
            "trust.no_signature_or_x509_pins",
            f"trust profile {profile_id!r} has no public-key or X.509 pins",
            path,
        )
    if args.environment is not None and environment != args.environment:
        _blocker(
            blockers,
            "trust.environment_mismatch",
            f"trust profile {profile_id!r} environment is {environment!r}, expected {args.environment!r}",
            path,
        )
    if policy != REQUIRE_VERIFIED:
        _blocker(
            blockers,
            "trust.policy_not_require_verified",
            f"trust profile {profile_id!r} uses {policy!r}",
            path,
        )
    if not crl_required:
        _blocker(
            blockers,
            "trust.crl_revocation_not_required",
            f"trust profile {profile_id!r} does not require CRL revocation checking",
            path,
        )
    elif x509_crl_count <= 0:
        _blocker(
            blockers,
            "trust.no_crl_revocation_material",
            f"trust profile {profile_id!r} requires CRL revocation checking but has no CRLs",
            path,
        )
    if not ocsp_required:
        _blocker(
            blockers,
            "trust.ocsp_revocation_not_required",
            f"trust profile {profile_id!r} does not require OCSP revocation checking",
            path,
        )
    elif x509_ocsp_response_count <= 0:
        _blocker(
            blockers,
            "trust.no_ocsp_revocation_material",
            f"trust profile {profile_id!r} requires OCSP revocation checking but has no OCSP responses",
            path,
        )
    result = {
        "profile_id": profile_id,
        "rail": rail,
        "environment": environment,
        "bundle_sha256": bundle_sha256,
        "source": {
            "authority": source_authority,
            "version": source_version,
            "url": source_url,
            "retrieved_at": source_retrieved_at_raw,
        },
        "embedded_signature_policy": policy,
        "signature_public_key_pin_count": signature_pin_count,
        "x509_trust_anchor_pin_count": x509_pin_count,
        "x509_trust_anchor_der": trust_anchor_der,
        "revoked_certificate_pin_count": revoked_pin_count,
        "revoked_certificate_der": revoked_der,
        "x509_required_certificate_policy_oid_count": policy_oid_count,
        "x509_require_crl_revocation_check": crl_required,
        "x509_crl_count": x509_crl_count,
        "x509_crl_der": x509_crl_der,
        "x509_require_ocsp_revocation_check": ocsp_required,
        "x509_ocsp_response_count": x509_ocsp_response_count,
        "x509_ocsp_response_der": x509_ocsp_response_der,
    }
    return result


def _verify_archive_receipts(
    summary: dict[str, Any],
    path: Path,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any] | None:
    receipt_summary = summary.get("receipt_verification")
    if receipt_summary is None:
        if args.allow_canary_stage_receipts_only:
            return None
        _blocker(
            blockers,
            "evidence.archive_receipts_not_reverified",
            "evidence summary does not include direct receipt archive verification",
            path,
        )
        return None
    receipt_obj = _require_object(receipt_summary, f"{path}.receipt_verification")
    return _verify_receipt_summary(
        receipt_obj,
        f"{path}.receipt_verification",
        path,
        blockers,
        missing_kinds_code="evidence.archive_receipt_kinds_missing",
        allow_failed_code="evidence.archive_receipts_allow_failed",
        allow_insecure_code="evidence.archive_receipts_insecure_http",
        allow_legacy_code="evidence.archive_receipts_allow_legacy_colr007",
        source_files_code="evidence.archive_receipts_source_files_not_required",
        count_mismatch_code="evidence.archive_receipt_count_mismatch",
        digest_missing_code="evidence.archive_receipt_digest_missing",
        duplicate_path_code="evidence.archive_receipt_path_duplicate",
        duplicate_digest_code="evidence.archive_receipt_digest_duplicate",
        kind_entry_mismatch_code="evidence.archive_receipt_kind_entry_mismatch",
    )


def _block_if_archive_receipts_do_not_cover_canaries(
    canaries: list[dict[str, Any]],
    archive_receipts: dict[str, Any] | None,
    path: Path,
    blockers: list[dict[str, Any]],
) -> None:
    """Require direct archive receipt verification to match canary receipt digests."""

    if archive_receipts is None:
        return
    archive_kinds_by_digest = {
        receipt["receipt_sha256"]: receipt.get("receipt_kind")
        for receipt in archive_receipts["receipts"]
        if _is_lower_sha256(receipt.get("receipt_sha256"))
    }
    canary_kinds_by_digest: dict[str, str] = {}
    for canary_offset, canary in enumerate(canaries):
        for receipt_offset, receipt in enumerate(canary["receipt_summary"]["receipts"]):
            receipt_sha256 = receipt.get("receipt_sha256")
            if not _is_lower_sha256(receipt_sha256):
                continue
            canary_kinds_by_digest[receipt_sha256] = receipt.get("receipt_kind")
            archive_kind = archive_kinds_by_digest.get(receipt_sha256)
            if archive_kind is None:
                _blocker(
                    blockers,
                    "evidence.archive_receipt_missing_canary_digest",
                    (
                        "direct receipt archive verification does not include "
                        f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                        f"[{receipt_offset}].receipt_sha256 {receipt_sha256}"
                    ),
                    path,
                )
                continue
            receipt_kind = receipt.get("receipt_kind")
            if archive_kind != receipt_kind:
                _blocker(
                    blockers,
                    "evidence.archive_receipt_canary_kind_mismatch",
                    (
                        "direct receipt archive verification binds "
                        f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                        f"[{receipt_offset}].receipt_sha256 {receipt_sha256} to "
                        f"receipt_kind {archive_kind!r}, not {receipt_kind!r}"
                    ),
                    path,
                )
    for receipt_offset, receipt in enumerate(archive_receipts["receipts"]):
        receipt_sha256 = receipt.get("receipt_sha256")
        if _is_lower_sha256(receipt_sha256) and receipt_sha256 not in canary_kinds_by_digest:
            _blocker(
                blockers,
                "evidence.archive_receipt_unreferenced_digest",
                (
                    "direct receipt archive verification includes "
                    f"receipt_verification.receipts[{receipt_offset}].receipt_sha256 "
                    f"{receipt_sha256} that no canary receipt_summary references"
                ),
                path,
            )


def _block_cross_canary_receipt_reuse(
    canaries: list[dict[str, Any]],
    path: Path,
    blockers: list[dict[str, Any]],
) -> None:
    """Block copied receipt evidence reused across distinct canary summaries."""

    seen_paths: dict[str, tuple[int, int]] = {}
    seen_digests: dict[str, tuple[int, int]] = {}
    for canary_offset, canary in enumerate(canaries):
        for receipt_offset, receipt in enumerate(canary["receipt_summary"]["receipts"]):
            receipt_path = receipt.get("path")
            if isinstance(receipt_path, str):
                if receipt_path in seen_paths:
                    first_canary, first_receipt = seen_paths[receipt_path]
                    _blocker(
                        blockers,
                        "evidence.canary_receipt_path_reused",
                        (
                            f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                            f"[{receipt_offset}].path duplicates canary_summaries"
                            f"[{first_canary}].receipt_summary.receipts"
                            f"[{first_receipt}].path"
                        ),
                        path,
                    )
                else:
                    seen_paths[receipt_path] = (canary_offset, receipt_offset)
            receipt_sha256 = receipt.get("receipt_sha256")
            if _is_lower_sha256(receipt_sha256):
                if receipt_sha256 in seen_digests:
                    first_canary, first_receipt = seen_digests[receipt_sha256]
                    _blocker(
                        blockers,
                        "evidence.canary_receipt_digest_reused",
                        (
                            f"canary_summaries[{canary_offset}].receipt_summary.receipts"
                            f"[{receipt_offset}].receipt_sha256 duplicates "
                            f"canary_summaries[{first_canary}].receipt_summary.receipts"
                            f"[{first_receipt}].receipt_sha256"
                        ),
                        path,
                    )
                else:
                    seen_digests[receipt_sha256] = (canary_offset, receipt_offset)


def verify_evidence_summary(
    path: Path,
    *,
    args: argparse.Namespace,
    blockers: list[dict[str, Any]],
) -> dict[str, Any]:
    """Verify one aggregate operator-evidence summary and append blockers."""

    summary = _require_object(_load_json(path), str(path))
    digest = _require_summary_digest(summary, str(path))
    _reject_unknown_keys(summary, EVIDENCE_SUMMARY_KEYS, str(path))
    verified_at, verified_at_dt = _require_timestamp(summary, "verified_at", str(path))
    _block_if_stale(
        verified_at_dt,
        max_age_days=args.max_evidence_age_days,
        code="evidence.summary_stale",
        label="evidence summary verified_at",
        path=path,
        blockers=blockers,
    )
    version = summary.get("version")
    if version != EVIDENCE_VERSION:
        raise ReadinessError(f"{path}.version must be {EVIDENCE_VERSION}")
    if not _require_bool(summary, "ok", str(path)):
        _blocker(blockers, "evidence.summary_not_ok", "evidence summary is not ok", path)
    evidence_policy = _verify_policy(summary, path, args, blockers)

    canary_summaries = _require_list(summary.get("canary_summaries"), f"{path}.canary_summaries")
    trust_summaries = _require_list(summary.get("trust_summaries"), f"{path}.trust_summaries")
    if not canary_summaries:
        _blocker(blockers, "evidence.no_canary_summaries", "no canary summaries recorded", path)
    if not trust_summaries:
        _blocker(blockers, "evidence.no_trust_summaries", "no trust summaries recorded", path)
    archive_receipts = _verify_archive_receipts(summary, path, args, blockers)

    canaries = [
        _verify_canary(
            _require_object(canary, f"{path}.canary_summaries[{offset}]"),
            f"{path}.canary_summaries[{offset}]",
            path,
            args,
            blockers,
        )
        for offset, canary in enumerate(canary_summaries)
    ]
    _block_if_archive_receipts_do_not_cover_canaries(
        canaries,
        archive_receipts,
        path,
        blockers,
    )
    _block_cross_canary_receipt_reuse(canaries, path, blockers)
    trust_outputs: list[dict[str, Any]] = []
    for offset, trust in enumerate(trust_summaries):
        label = f"{path}.trust_summaries[{offset}]"
        trust_obj = _require_object(trust, label)
        _reject_unknown_keys(trust_obj, TRUST_SUMMARY_KEYS, label)
        trust_path = _validate_compact_summary_path(
            _require_string(trust_obj, "path", label),
            f"{label}.path",
        )
        verified_at_raw, verified_at_dt = _require_timestamp(trust_obj, "verified_at", label)
        _block_if_stale(
            verified_at_dt,
            max_age_days=args.max_trust_age_days,
            code="trust.summary_stale",
            label="trust summary verified_at",
            path=path,
            blockers=blockers,
        )
        summary_sha256 = _require_sha256(trust_obj, SUMMARY_DIGEST_FIELD, label)
        verified_bundles = _require_positive_int(trust_obj, "verified_bundles", label)
        profile_json_emitted = _require_bool(trust_obj, "profile_json_emitted", label)
        profile_json_emittable = _require_bool(trust_obj, "profile_json_emittable", label)
        if profile_json_emitted:
            profile_json_sha256 = _require_sha256(trust_obj, "profile_json_sha256", label)
        else:
            if "profile_json_sha256" not in trust_obj:
                raise ReadinessError(
                    f"{label}.profile_json_sha256 must be null when profile JSON was not emitted"
                )
            if trust_obj["profile_json_sha256"] is not None:
                raise ReadinessError(
                    f"{label}.profile_json_sha256 must be null when profile JSON was not emitted"
                )
            profile_json_sha256 = None
        if not profile_json_emitted:
            _blocker(
                blockers,
                "trust.profile_json_not_emitted",
                "trust summary did not emit profile override JSON",
                path,
            )
        if not profile_json_emittable:
            _blocker(
                blockers,
                "trust.profile_json_not_emittable",
                "trust summary cannot emit production profile override JSON",
                path,
            )
        profiles_raw = _require_list(trust_obj.get("profiles"), f"{label}.profiles")
        if not profiles_raw:
            _blocker(blockers, "trust.no_profiles", "trust summary has no profiles", path)
        if len(profiles_raw) != verified_bundles:
            _blocker(
                blockers,
                "trust.profile_count_mismatch",
                "trust profile count does not match verified_bundles",
                path,
            )
        profiles = [
            _verify_trust_profile(
                _require_object(profile, f"{label}.profiles[{profile_offset}]"),
                f"{label}.profiles[{profile_offset}]",
                path,
                args,
                blockers,
            )
            for profile_offset, profile in enumerate(profiles_raw)
        ]
        seen_profile_ids: dict[str, int] = {}
        seen_bundle_digests: dict[str, int] = {}
        for profile_offset, profile in enumerate(profiles):
            profile_id = profile["profile_id"]
            if profile_id in seen_profile_ids:
                _blocker(
                    blockers,
                    "trust.profile_id_duplicate",
                    (
                        f"{label}.profiles[{profile_offset}].profile_id duplicates "
                        f"{label}.profiles[{seen_profile_ids[profile_id]}].profile_id"
                    ),
                    path,
                )
            else:
                seen_profile_ids[profile_id] = profile_offset
            bundle_sha256 = profile["bundle_sha256"]
            if bundle_sha256 in seen_bundle_digests:
                _blocker(
                    blockers,
                    "trust.bundle_digest_duplicate",
                    (
                        f"{label}.profiles[{profile_offset}].bundle_sha256 duplicates "
                        f"{label}.profiles[{seen_bundle_digests[bundle_sha256]}].bundle_sha256"
                    ),
                    path,
                )
            else:
                seen_bundle_digests[bundle_sha256] = profile_offset
        trust_outputs.append(
            {
                "path": trust_path,
                "verified_at": verified_at_raw,
                "verified_bundles": verified_bundles,
                "profile_json_emitted": profile_json_emitted,
                "profile_json_emittable": profile_json_emittable,
                "profile_json_sha256": profile_json_sha256,
                "profiles": profiles,
                "summary_sha256": summary_sha256,
            }
        )
    _reject_duplicate_compact_summaries(canaries, f"{path}.canary_summaries")
    _reject_duplicate_compact_summaries(trust_outputs, f"{path}.trust_summaries")
    return {
        "path": str(path),
        "verified_at": verified_at,
        "policy": evidence_policy,
        "canary_summaries": canaries,
        "trust_summaries": trust_outputs,
        "receipt_verification": archive_receipts,
        "summary_sha256": digest,
    }


def run(args: argparse.Namespace) -> int:
    if not args.xsd_summary:
        raise ReadinessError("provide at least one --xsd-summary")
    if not args.evidence_summary:
        raise ReadinessError("provide at least one --evidence-summary")
    args.provider = _require_cli_string(args.provider, "--provider")
    args.environment = _require_cli_string(args.environment, "--environment")
    args.max_xsd_age_days = _require_positive_cli_int(
        args.max_xsd_age_days,
        "--max-xsd-age-days",
    )
    args.max_evidence_age_days = _require_positive_cli_int(
        args.max_evidence_age_days,
        "--max-evidence-age-days",
    )
    args.max_canary_age_days = _require_positive_cli_int(
        args.max_canary_age_days,
        "--max-canary-age-days",
    )
    args.max_trust_age_days = _require_positive_cli_int(
        args.max_trust_age_days,
        "--max-trust-age-days",
    )
    args.max_trust_source_age_days = _require_positive_cli_int(
        args.max_trust_source_age_days,
        "--max-trust-source-age-days",
    )

    blockers: list[dict[str, Any]] = []
    warnings: list[dict[str, Any]] = []
    xsd_paths = list(args.xsd_summary)
    evidence_paths = list(args.evidence_summary)
    _reject_duplicate_paths([path.resolve() for path in xsd_paths], "--xsd-summary")
    _reject_duplicate_paths([path.resolve() for path in evidence_paths], "--evidence-summary")
    xsd_summaries = [
        verify_xsd_summary(
            path,
            allow_reviewed_xsd_gaps=args.allow_reviewed_xsd_gaps,
            max_age_days=args.max_xsd_age_days,
            blockers=blockers,
            warnings=warnings,
        )
        for path in xsd_paths
    ]
    evidence_summaries = [
        verify_evidence_summary(path, args=args, blockers=blockers)
        for path in evidence_paths
    ]
    _reject_duplicate_compact_summaries(xsd_summaries, "xsd_summaries")
    _reject_duplicate_compact_summaries(evidence_summaries, "evidence_summaries")
    output: dict[str, Any] = {
        "version": READINESS_VERSION,
        "checked_at": dt.datetime.now(dt.UTC).replace(microsecond=0).isoformat(),
        "ok": not blockers,
        "blockers": blockers,
        "warnings": warnings,
        "xsd_summaries": xsd_summaries,
        "evidence_summaries": evidence_summaries,
        "policy": {
            "provider": args.provider,
            "environment": args.environment,
            "allow_reviewed_xsd_gaps": args.allow_reviewed_xsd_gaps,
            "allow_canary_stage_receipts_only": args.allow_canary_stage_receipts_only,
            "max_xsd_age_days": args.max_xsd_age_days,
            "max_evidence_age_days": args.max_evidence_age_days,
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
    return 0 if output["ok"] else 1


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Aggregate ISO 20022 production-readiness evidence summaries."
    )
    parser.add_argument(
        "--xsd-summary",
        action="append",
        default=[],
        type=Path,
        help="Digest-bound summary JSON from iso_xsd_fixture_verify.py; repeatable.",
    )
    parser.add_argument(
        "--evidence-summary",
        action="append",
        default=[],
        type=Path,
        help="Digest-bound summary JSON from iso_operator_evidence_verify.py; repeatable.",
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
        help="Optional path to write the production-readiness summary JSON.",
    )
    parser.add_argument(
        "--max-xsd-age-days",
        type=int,
        help="Maximum age in days for XSD fixture summaries.",
    )
    parser.add_argument(
        "--max-evidence-age-days",
        type=int,
        help="Maximum age in days for aggregate operator evidence summaries.",
    )
    parser.add_argument(
        "--max-canary-age-days",
        type=int,
        help="Maximum age in days for compact canary finished_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-age-days",
        type=int,
        help="Maximum age in days for compact trust-summary verified_at timestamps.",
    )
    parser.add_argument(
        "--max-trust-source-age-days",
        type=int,
        help="Maximum age in days for trust source retrieved_at timestamps recorded by the evidence gate.",
    )
    parser.add_argument(
        "--allow-reviewed-xsd-gaps",
        action="store_true",
        help="Downgrade reviewed XSD missing-schema/schema-only gaps to warnings for local audits.",
    )
    parser.add_argument(
        "--allow-canary-stage-receipts-only",
        action="store_true",
        help="Do not require final evidence summaries to include direct receipt archive verification.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except ReadinessError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
