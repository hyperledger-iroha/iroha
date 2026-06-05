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
import os
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
DEFAULT_RESPONSE_LIMIT_BYTES = 64 * 1024
INDEX_DIGEST_FIELD = "index_sha256"
INDEX_FILE = "messages.index.json"
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


class AdapterError(RuntimeError):
    """Raised when an audit preimage or publication response is invalid."""


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
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise AdapterError(f"JSON object contains duplicate key {key!r}")
        seen.add(key)
        result[key] = value
    return result


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise AdapterError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _load_json_bytes(path: Path) -> tuple[Any, bytes]:
    raw = _read_regular_file(path)
    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        ), raw
    except UnicodeDecodeError as error:
        raise AdapterError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise AdapterError(f"{path} is not valid JSON: {error}") from error


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


def _verify_persisted_history_entry(value: Any, label: str) -> tuple[str, str]:
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_HISTORY_KEYS, label)
    status = _require_clean_string(value.get("status"), f"{label}.status")
    if status not in {"Pending", "Accepted", "Rejected"}:
        raise AdapterError(f"{label}.status must be Pending, Accepted, or Rejected")
    code = _require_pacs002_code(value.get("pacs002_code"), f"{label}.pacs002_code")
    _require_nonnegative_int(value.get("updated_at_ms"), f"{label}.updated_at_ms")
    _require_optional_clean_string(value.get("detail"), f"{label}.detail")
    _require_optional_clean_string(value.get("reason_code"), f"{label}.reason_code")
    return status, code


def _verify_persisted_record_source(
    index_record: dict[str, Any],
    path: Path,
    label: str,
) -> None:
    value = _load_json(path)
    if not isinstance(value, dict):
        raise AdapterError(f"{label} must contain a JSON object")
    _reject_unknown_keys(value, PERSISTED_RECORD_KEYS, label)
    if value.get("version") != PERSISTED_RECORD_VERSION:
        raise AdapterError(f"{label} has unsupported persisted record version")
    source_digest = require_digest_matches(value, PERSISTED_RECORD_DIGEST_FIELD, label)
    if source_digest != index_record.get(PERSISTED_RECORD_DIGEST_FIELD):
        raise AdapterError(f"{label} record_sha256 does not match audit index record")
    if value.get("message_id") != index_record.get("message_id"):
        raise AdapterError(f"{label}.message_id does not match audit index record")
    if value.get("state") != index_record.get("state"):
        raise AdapterError(f"{label}.state does not match audit index record")
    _require_nonnegative_int(value.get("updated_at_ms"), f"{label}.updated_at_ms")
    if value.get("updated_at_ms") != index_record.get("updated_at_ms"):
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
    for offset, entry in enumerate(history):
        last_status, last_code = _verify_persisted_history_entry(
            entry,
            f"{label}.status_history[{offset}]",
        )
    derived_code = _derived_pacs002_code(value, label)
    if derived_code != index_record.get("pacs002_code"):
        raise AdapterError(f"{label}.pacs002_code does not match persisted state")
    if last_status != value.get("state") or last_code != derived_code:
        raise AdapterError(f"{label}.status_history does not end with current status")


def _record_store_dir(anchor: dict[str, Any], label: str) -> Path | None:
    store_dir = anchor.get("store_dir")
    if store_dir is None:
        return None
    return Path(_require_clean_string(store_dir, f"{label}.store_dir"))


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


def verify_audit_index(index: Any) -> dict[str, Any]:
    """Verify the exported audit index digest and basic record-count shape."""

    if not isinstance(index, dict):
        raise AdapterError("audit index must be a JSON object")
    _reject_unknown_keys(index, AUDIT_INDEX_KEYS, "audit index")
    require_digest_matches(index, INDEX_DIGEST_FIELD, "audit index")
    record_count = index.get("record_count")
    records = index.get("records")
    if not isinstance(record_count, int) or record_count < 0:
        raise AdapterError("audit index record_count must be a non-negative integer")
    if not isinstance(records, list):
        raise AdapterError("audit index records must be an array")
    if len(records) != record_count:
        raise AdapterError(
            f"audit index record_count {record_count} does not match records length {len(records)}"
        )
    for offset, record in enumerate(records):
        _verify_audit_index_record(record, f"audit index records[{offset}]")
    return index


def verify_anchor_file(
    export_dir: Path,
    anchor_path: Path,
    *,
    allow_missing_record_sources: bool = False,
) -> VerifiedAnchor:
    """Verify one notary anchor against the export directory index file."""

    anchor_value, raw = _load_json_bytes(anchor_path)
    if not isinstance(anchor_value, dict):
        raise AdapterError(f"{anchor_path} must contain a JSON object")
    _reject_unknown_keys(anchor_value, ANCHOR_KEYS, str(anchor_path))
    if anchor_value.get("version") != ANCHOR_VERSION:
        raise AdapterError(f"{anchor_path} has unsupported anchor version")
    anchor_sha256 = require_digest_matches(anchor_value, ANCHOR_DIGEST_FIELD, str(anchor_path))

    audit_index = verify_audit_index(anchor_value.get("audit_index"))
    index_sha256 = anchor_value.get(INDEX_DIGEST_FIELD)
    embedded_index_sha256 = audit_index.get(INDEX_DIGEST_FIELD)
    if index_sha256 != embedded_index_sha256:
        raise AdapterError(
            f"{anchor_path} index_sha256 does not match embedded audit index digest"
        )
    if anchor_value.get("record_count") != audit_index.get("record_count"):
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
        if _read_regular_file(digest_anchor) != raw:
            raise AdapterError(f"{latest} differs from digest-addressed peer {digest_anchor}")

    return VerifiedAnchor(
        path=anchor_path,
        payload=anchor_value,
        raw=raw,
        index_sha256=index_sha256,
        anchor_sha256=anchor_sha256,
        record_count=anchor_value["record_count"],
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
    if any(segment in {".", ".."} for segment in path.split("/")):
        raise AdapterError(f"{label} path must not contain dot segments")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise AdapterError(f"{label} path must not contain encoded dot or separator characters")
    if "%25" in lowered:
        raise AdapterError(f"{label} path must not contain encoded percent characters")


def _validate_endpoint(endpoint: str, allow_insecure_http: bool) -> None:
    _reject_url_control_chars(endpoint, "endpoint")
    _reject_url_percent_encoding_smuggling(endpoint, "endpoint")
    if endpoint != endpoint.strip():
        raise AdapterError("endpoint must not have surrounding whitespace")
    if any(ch.isspace() for ch in endpoint):
        raise AdapterError("endpoint must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(endpoint)
        hostname = parsed.hostname
    except ValueError as error:
        raise AdapterError(f"endpoint {endpoint} is not a valid URL: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise AdapterError(
                f"refusing insecure HTTP endpoint {endpoint}; pass --allow-insecure-http for local tests"
            )
        raise AdapterError(f"endpoint {endpoint} must use http or https")
    _validate_url_port(parsed, f"endpoint {endpoint}")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise AdapterError(f"endpoint {endpoint} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise AdapterError(f"endpoint {endpoint} must not contain credentials")
    _validate_url_host(parsed, f"endpoint {endpoint}")
    if parsed.params or parsed.query or parsed.fragment:
        raise AdapterError(
            f"endpoint {endpoint} must not contain params, query, or fragment"
        )
    _validate_url_path(parsed, f"endpoint {endpoint}")


def _reject_duplicate_endpoints(endpoints: list[str]) -> None:
    seen: dict[str, int] = {}
    for offset, endpoint in enumerate(endpoints):
        if endpoint in seen:
            raise AdapterError(
                f"--endpoint[{offset}] duplicates --endpoint[{seen[endpoint]}]: {endpoint}"
            )
        seen[endpoint] = offset


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
        with urllib.request.urlopen(request, timeout=timeout_secs) as response:
            body = response.read(response_limit_bytes + 1)
            if len(body) > response_limit_bytes:
                raise AdapterError(
                    f"{endpoint} response exceeded {response_limit_bytes} byte limit"
                )
            status_code = int(response.status)
    except urllib.error.HTTPError as error:
        try:
            body = error.read(response_limit_bytes + 1)
        finally:
            error.close()
        if len(body) > response_limit_bytes:
            raise AdapterError(f"{endpoint} error response exceeded {response_limit_bytes} byte limit")
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
            error=str(error.reason),
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
    return body[:4096].decode("utf-8", errors="replace")


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
                timeout_secs=args.timeout_secs,
                response_limit_bytes=args.response_limit_bytes,
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
    args = parser.parse_args(argv)
    try:
        return run(args)
    except AdapterError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
