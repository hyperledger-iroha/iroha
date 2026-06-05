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
import stat
import sys
import urllib.parse
from pathlib import Path
from typing import Any


ANCHOR_DIGEST_FIELD = "anchor_sha256"
ANCHOR_DIR = "anchors"
ANCHOR_VERSION = 1
INDEX_DIGEST_FIELD = "index_sha256"
INDEX_FILE = "messages.index.json"
LATEST_ANCHOR_FILE = "latest.notary.json"
PERSISTED_RECORD_DIGEST_FIELD = "record_sha256"
PERSISTED_RECORD_VERSION = 1
RECORDS_DIR = "messages"
RECEIPT_DIGEST_FIELD = "receipt_sha256"
RECEIPT_VERSION = 1
SUMMARY_DIGEST_FIELD = "summary_sha256"
SUPPORTED_KINDS = {"iso-audit-notary", "iso-rail-gateway"}
LEGACY_RAIL_MESSAGE_TYPES = {"colr.007"}
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


class ReceiptError(RuntimeError):
    """Raised when an operator receipt is invalid."""


def sha256_hex(data: bytes) -> str:
    """Return a lowercase SHA-256 hex digest."""

    return hashlib.sha256(data).hexdigest()


def _canonical_json_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode("utf-8")


def _canonical_summary_json_bytes(value: Any) -> bytes:
    return json.dumps(
        value,
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


def _read_regular_file(path: Path) -> bytes:
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{path} must not be a symlink")
    if not stat.S_ISREG(metadata.st_mode):
        raise ReceiptError(f"{path} must be a regular file")
    flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
    fd = -1
    try:
        fd = os.open(path, flags)
        if not stat.S_ISREG(os.fstat(fd).st_mode):
            raise ReceiptError(f"{path} must be a regular file")
        with os.fdopen(fd, "rb") as handle:
            fd = -1
            return handle.read()
    except FileNotFoundError as error:
        raise ReceiptError(f"{path} does not exist") from error
    except OSError as error:
        if error.errno == errno.ELOOP:
            raise ReceiptError(f"{path} must not be a symlink") from error
        raise ReceiptError(f"cannot open {path} for reading: {error.strerror}") from error
    finally:
        if fd >= 0:
            os.close(fd)


def _load_json(path: Path) -> Any:
    raw = _read_regular_file(path)
    try:
        return json.loads(
            raw.decode("utf-8"),
            object_pairs_hook=_reject_duplicate_json_keys,
        )
    except UnicodeDecodeError as error:
        raise ReceiptError(f"{path} is not UTF-8 JSON") from error
    except json.JSONDecodeError as error:
        raise ReceiptError(f"{path} is not valid JSON: {error}") from error


def _reject_unknown_keys(value: dict[str, Any], allowed: set[str], label: str) -> None:
    unknown = sorted(set(value) - allowed)
    if unknown:
        raise ReceiptError(f"{label} contains unknown keys: {', '.join(unknown)}")


def _reject_duplicate_json_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    seen: set[str] = set()
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in seen:
            raise ReceiptError(f"JSON object contains duplicate key {key!r}")
        seen.add(key)
        result[key] = value
    return result


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
    actual = digest_without_field(obj, digest_field)
    if actual != expected:
        raise ReceiptError(f"{label} {digest_field} mismatch: expected {expected}, got {actual}")
    return expected


def _check_no_secret_material(receipt: dict[str, Any], path: Path) -> None:
    forbidden = {
        "authorization",
        "bearer_token",
        "token",
        "private_key",
        "secret",
        "x-iroha-signature",
    }
    for key, value in receipt.items():
        lowered = key.lower()
        if lowered in forbidden or any(part in lowered for part in ("authorization", "token", "secret", "private_key")):
            raise ReceiptError(f"{path} contains forbidden secret-looking field {key}")
        if isinstance(value, str) and value.lower().startswith("bearer "):
            raise ReceiptError(f"{path} contains bearer-token material in field {key}")


def _reject_url_control_chars(url: str, label: str) -> None:
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in url):
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


def _validate_url_host(parsed: urllib.parse.ParseResult, label: str) -> None:
    raw_host = _raw_url_host(parsed)
    if "%" in raw_host:
        raise ReceiptError(f"{label} host must not contain percent escapes")
    if raw_host != raw_host.lower():
        raise ReceiptError(f"{label} host must be lowercase")
    if raw_host.endswith("."):
        raise ReceiptError(f"{label} host must not end with a dot")
    try:
        ipaddress.ip_address(raw_host)
        return
    except ValueError:
        pass
    if ":" in raw_host:
        raise ReceiptError(f"{label} host must be a valid IP address")
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


def _validate_url_path(parsed: urllib.parse.ParseResult, label: str) -> None:
    path = parsed.path
    if "\\" in path:
        raise ReceiptError(f"{label} path must use forward slashes")
    if ";" in path:
        raise ReceiptError(f"{label} path must not contain semicolon parameters")
    if any(segment in {".", ".."} for segment in path.split("/")):
        raise ReceiptError(f"{label} path must not contain dot segments")
    lowered = path.lower()
    if any(token in lowered for token in ("%2e", "%2f", "%5c")):
        raise ReceiptError(f"{label} path must not contain encoded dot or separator characters")
    if "%25" in lowered:
        raise ReceiptError(f"{label} path must not contain encoded percent characters")


def _require_clean_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ReceiptError(f"{label} must be a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise ReceiptError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    return value


def _require_optional_clean_string(value: Any, label: str) -> str | None:
    if value is None:
        return None
    return _require_clean_string(value, label)


def _require_nonnegative_int(value: Any, label: str) -> int:
    if isinstance(value, bool) or not isinstance(value, int) or value < 0:
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


def _derived_pacs002_code(record: dict[str, Any], label: str) -> str:
    state = _require_clean_string(record.get("state"), f"{label}.state")
    if state not in {"Pending", "Accepted", "Rejected"}:
        raise ReceiptError(f"{label}.state must be Pending, Accepted, or Rejected")
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
        raise ReceiptError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_CONTEXT_KEYS, label)
    _verify_optional_clean_string_fields(value, PERSISTED_CONTEXT_KEYS, label)


def _verify_persisted_metadata(
    value: Any, label: str, index_record: dict[str, Any]
) -> None:
    if not isinstance(value, dict):
        raise ReceiptError(f"{label} must be an object")
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
        raise ReceiptError(f"{label}.payload_hash must be a canonical SHA-256")
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


def _verify_persisted_history_entry(value: Any, label: str) -> tuple[str, str]:
    if not isinstance(value, dict):
        raise ReceiptError(f"{label} must be an object")
    _reject_unknown_keys(value, PERSISTED_HISTORY_KEYS, label)
    status = _require_clean_string(value.get("status"), f"{label}.status")
    if status not in {"Pending", "Accepted", "Rejected"}:
        raise ReceiptError(f"{label}.status must be Pending, Accepted, or Rejected")
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
        raise ReceiptError(f"{label} must contain a JSON object")
    _reject_unknown_keys(value, PERSISTED_RECORD_KEYS, label)
    if value.get("version") != PERSISTED_RECORD_VERSION:
        raise ReceiptError(f"{label} has unsupported persisted record version")
    source_digest = require_digest_matches(value, PERSISTED_RECORD_DIGEST_FIELD, label)
    if source_digest != index_record.get(PERSISTED_RECORD_DIGEST_FIELD):
        raise ReceiptError(f"{label} record_sha256 does not match audit index record")
    if value.get("message_id") != index_record.get("message_id"):
        raise ReceiptError(f"{label}.message_id does not match audit index record")
    if value.get("state") != index_record.get("state"):
        raise ReceiptError(f"{label}.state does not match audit index record")
    _require_nonnegative_int(value.get("updated_at_ms"), f"{label}.updated_at_ms")
    if value.get("updated_at_ms") != index_record.get("updated_at_ms"):
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
        _require_optional_clean_string(value.get(key), f"{label}.{key}")
    if value.get("transaction_hash") != index_record.get("transaction_hash"):
        raise ReceiptError(f"{label}.transaction_hash does not match audit index record")
    _require_bool(value.get("ledger_tx_queued"), f"{label}.ledger_tx_queued")
    change_reason_codes = value.get("change_reason_codes")
    if not isinstance(change_reason_codes, list):
        raise ReceiptError(f"{label}.change_reason_codes must be an array")
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
        raise ReceiptError(f"{label}.status_history must be a non-empty array")
    last_status = None
    last_code = None
    for offset, entry in enumerate(history):
        last_status, last_code = _verify_persisted_history_entry(
            entry,
            f"{label}.status_history[{offset}]",
        )
    derived_code = _derived_pacs002_code(value, label)
    if derived_code != index_record.get("pacs002_code"):
        raise ReceiptError(f"{label}.pacs002_code does not match persisted state")
    if last_status != value.get("state") or last_code != derived_code:
        raise ReceiptError(f"{label}.status_history does not end with current status")


def _require_https(url: str, *, allow_insecure_http: bool, label: str) -> None:
    _reject_url_control_chars(url, label)
    _reject_url_percent_encoding_smuggling(url, label)
    if url != url.strip():
        raise ReceiptError(f"{label} URL must not have surrounding whitespace")
    if any(ch.isspace() for ch in url):
        raise ReceiptError(f"{label} URL must not contain whitespace")
    try:
        parsed = urllib.parse.urlparse(url)
        hostname = parsed.hostname
    except ValueError as error:
        raise ReceiptError(f"{label} URL {url} is not valid: {error}") from error
    if parsed.scheme != "https" and not (
        parsed.scheme == "http" and allow_insecure_http
    ):
        if parsed.scheme == "http":
            raise ReceiptError(f"{label} uses insecure HTTP URL {url}")
        raise ReceiptError(f"{label} must use http or https URL, got {url}")
    try:
        port = parsed.port
    except ValueError as error:
        raise ReceiptError(f"{label} URL {url} has invalid port: {error}") from error
    if (parsed.scheme == "https" and port == 443) or (
        parsed.scheme == "http" and port == 80
    ):
        raise ReceiptError(f"{label} URL {url} must not explicitly specify the default port")
    if not parsed.netloc or hostname is None or not hostname.strip():
        raise ReceiptError(f"{label} URL {url} must include a host")
    if parsed.username is not None or parsed.password is not None:
        raise ReceiptError(f"{label} URL {url} must not contain credentials")
    _validate_url_host(parsed, f"{label} URL {url}")
    if parsed.params or parsed.query or parsed.fragment:
        raise ReceiptError(f"{label} URL {url} must not contain params, query, or fragment")
    _validate_url_path(parsed, f"{label} URL {url}")


def _check_status(receipt: dict[str, Any], path: Path, *, allow_failed: bool) -> None:
    ok = receipt.get("ok")
    status_code = receipt.get("status_code")
    if not isinstance(ok, bool):
        raise ReceiptError(f"{path} ok must be boolean")
    if status_code is not None and (not isinstance(status_code, int) or status_code < 100):
        raise ReceiptError(f"{path} status_code must be null or an HTTP status integer")
    success = isinstance(status_code, int) and 200 <= status_code <= 299
    if ok != success:
        raise ReceiptError(f"{path} ok does not match status_code success state")
    if not allow_failed and not success:
        raise ReceiptError(f"{path} is not a successful 2xx receipt")


def _check_timestamp(receipt: dict[str, Any], key: str, path: Path) -> None:
    value = _require_clean_string(receipt.get(key), f"{path} {key}")
    try:
        parsed = dt.datetime.fromisoformat(value)
    except ValueError as error:
        raise ReceiptError(f"{path} {key} is not a valid ISO timestamp") from error
    if parsed.tzinfo is None or parsed.utcoffset() is None:
        raise ReceiptError(f"{path} {key} must include a timezone offset")


def _check_endpoint_digest(receipt: dict[str, Any], path: Path, endpoint: str) -> None:
    endpoint_sha256 = receipt.get("endpoint_sha256")
    if not _is_lower_hex_sha256(endpoint_sha256):
        raise ReceiptError(f"{path} has invalid endpoint_sha256")
    actual = sha256_hex(endpoint.encode("utf-8"))
    if endpoint_sha256 != actual:
        raise ReceiptError(f"{path} endpoint_sha256 does not match endpoint")


def _check_response_metadata(receipt: dict[str, Any], path: Path) -> None:
    response_body_sha256 = receipt.get("response_body_sha256")
    response_body_preview = receipt.get("response_body_preview")
    if response_body_sha256 is None:
        if response_body_preview is not None:
            raise ReceiptError(f"{path} response_body_preview requires response_body_sha256")
    else:
        if not _is_lower_hex_sha256(response_body_sha256):
            raise ReceiptError(f"{path} has invalid response_body_sha256")
        if not isinstance(response_body_preview, str):
            raise ReceiptError(f"{path} response_body_preview must be a string")
        if len(response_body_preview) > 4096:
            raise ReceiptError(f"{path} response_body_preview exceeds 4096 characters")
        if "bearer " in response_body_preview.lower():
            raise ReceiptError(f"{path} response_body_preview contains bearer-token material")

    error = receipt.get("error")
    if error is not None:
        _normalize_optional_string(error, f"{path} error")
    if receipt.get("ok") and error is not None:
        raise ReceiptError(f"{path} successful receipt must not record error")


def _verify_audit_index_record_source(record: Any, label: str) -> None:
    if not isinstance(record, dict):
        raise ReceiptError(f"{label} must be an object")
    _reject_unknown_keys(record, AUDIT_INDEX_RECORD_KEYS, label)
    message_id = _require_clean_string(record.get("message_id"), f"{label}.message_id")
    filename = _require_clean_string(record.get("filename"), f"{label}.filename")
    expected_filename = _expected_message_filename(message_id)
    if filename != expected_filename:
        raise ReceiptError(
            f"{label}.filename must be digest-addressed as {expected_filename}"
        )
    if not _is_lower_hex_sha256(record.get("record_sha256")):
        raise ReceiptError(f"{label}.record_sha256 must be a canonical SHA-256")
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
        raise ReceiptError(f"{label}.payload_hash must be a canonical SHA-256")
    _require_optional_clean_string(
        record.get("reference_snapshot_id"),
        f"{label}.reference_snapshot_id",
    )


def _verify_audit_index_source(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReceiptError(f"{label} must contain a JSON object")
    _reject_unknown_keys(value, AUDIT_INDEX_KEYS, label)
    require_digest_matches(value, INDEX_DIGEST_FIELD, label)
    record_count = value.get("record_count")
    records = value.get("records")
    if not isinstance(record_count, int) or record_count < 0:
        raise ReceiptError(f"{label} record_count must be a non-negative integer")
    if not isinstance(records, list):
        raise ReceiptError(f"{label} records must be an array")
    if len(records) != record_count:
        raise ReceiptError(f"{label} record_count does not match records length")
    for offset, record in enumerate(records):
        _verify_audit_index_record_source(record, f"{label}.records[{offset}]")
    return value


def _ensure_input_directory(path: Path, label: str) -> None:
    try:
        metadata = path.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{label} {path} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{label} {path} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise ReceiptError(f"{label} {path} must be a directory")


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
    require_source_files: bool,
) -> None:
    records = audit_index.get("records", [])
    if store_dir is None:
        if require_source_files and records:
            raise ReceiptError(f"{label}.store_dir is required to verify audit records")
        return
    if not store_dir.exists():
        if require_source_files:
            raise ReceiptError(f"{label}.store_dir {store_dir} does not exist")
        return
    _ensure_input_directory(store_dir, f"{label}.store_dir")
    messages_dir = store_dir / RECORDS_DIR
    if not messages_dir.exists():
        if require_source_files:
            raise ReceiptError(
                f"{label}.store_dir/{RECORDS_DIR} {messages_dir} does not exist"
            )
        return
    _ensure_input_directory(messages_dir, f"{label}.store_dir/{RECORDS_DIR}")
    for offset, record in enumerate(records):
        if not isinstance(record, dict):
            raise ReceiptError(f"{label}.records[{offset}] must be an object")
        record_path = messages_dir / record["filename"]
        if not record_path.exists():
            if require_source_files:
                raise ReceiptError(f"{label} references missing audit record {record_path}")
            continue
        _verify_persisted_record_source(
            record,
            record_path,
            f"{record_path}",
        )


def _anchor_export_dir_for_convention(
    anchor_path: Path,
    *,
    index_sha256: str,
    receipt_path: Path,
) -> Path:
    if anchor_path.name == LATEST_ANCHOR_FILE:
        return anchor_path.parent
    if anchor_path.parent.name == ANCHOR_DIR:
        expected_name = f"{index_sha256}.notary.json"
        if anchor_path.name != expected_name:
            raise ReceiptError(
                f"{receipt_path} anchor_path filename must be digest-addressed as {expected_name}"
            )
        return anchor_path.parent.parent
    raise ReceiptError(
        f"{receipt_path} anchor_path must be {LATEST_ANCHOR_FILE} or {ANCHOR_DIR}/<index_sha256>.notary.json"
    )


def _verify_anchor_path_peers(
    anchor_path: Path,
    export_dir: Path,
    *,
    index_sha256: str,
    receipt_path: Path,
    require_source_files: bool,
) -> None:
    if anchor_path.name != LATEST_ANCHOR_FILE:
        return
    digest_anchor = export_dir / ANCHOR_DIR / f"{index_sha256}.notary.json"
    if digest_anchor.exists():
        if _read_regular_file(digest_anchor) != _read_regular_file(anchor_path):
            raise ReceiptError(
                f"{receipt_path} latest anchor differs from digest-addressed peer"
            )
    elif require_source_files:
        raise ReceiptError(
            f"{receipt_path} latest anchor has no digest-addressed peer {digest_anchor}"
        )


def _verify_anchor_source(receipt: dict[str, Any], path: Path, *, require_source_files: bool) -> None:
    anchor_sha256 = receipt.get(ANCHOR_DIGEST_FIELD)
    index_sha256 = receipt.get(INDEX_DIGEST_FIELD)
    if not _is_lower_hex_sha256(anchor_sha256):
        raise ReceiptError(f"{path} has invalid anchor_sha256")
    if not _is_lower_hex_sha256(index_sha256):
        raise ReceiptError(f"{path} has invalid index_sha256")
    record_count = receipt.get("record_count")
    if not isinstance(record_count, int) or record_count < 0:
        raise ReceiptError(f"{path} record_count must be a non-negative integer")

    anchor_path_raw = _require_clean_string(
        receipt.get("anchor_path"),
        f"{path} anchor_path",
    )
    anchor_path = Path(anchor_path_raw)
    export_dir = _anchor_export_dir_for_convention(
        anchor_path,
        index_sha256=index_sha256,
        receipt_path=path,
    )
    if anchor_path.is_symlink():
        raise ReceiptError(f"{anchor_path} must not be a symlink")
    if not anchor_path.exists():
        if require_source_files:
            raise ReceiptError(f"{path} references missing anchor_path {anchor_path}")
        return

    anchor = _load_json(anchor_path)
    if not isinstance(anchor, dict):
        raise ReceiptError(f"{anchor_path} must contain a JSON object")
    _reject_unknown_keys(anchor, ANCHOR_KEYS, str(anchor_path))
    if anchor.get("version") != ANCHOR_VERSION:
        raise ReceiptError(f"{anchor_path} has unsupported anchor version")
    if (
        require_digest_matches(anchor, ANCHOR_DIGEST_FIELD, str(anchor_path))
        != anchor_sha256
    ):
        raise ReceiptError(f"{path} anchor_sha256 does not match source anchor")
    if anchor.get(INDEX_DIGEST_FIELD) != index_sha256:
        raise ReceiptError(f"{path} index_sha256 does not match source anchor")
    if anchor.get("record_count") != record_count:
        raise ReceiptError(f"{path} record_count does not match source anchor")
    audit_index = _verify_audit_index_source(
        anchor.get("audit_index"),
        f"{anchor_path}.audit_index",
    )
    if audit_index.get(INDEX_DIGEST_FIELD) != index_sha256:
        raise ReceiptError(f"{path} index_sha256 does not match embedded audit index")
    if audit_index.get("record_count") != record_count:
        raise ReceiptError(f"{path} record_count does not match embedded audit index")
    _verify_persisted_record_sources(
        audit_index,
        _record_store_dir(anchor, str(anchor_path)),
        str(anchor_path),
        require_source_files=require_source_files,
    )
    _verify_anchor_path_peers(
        anchor_path,
        export_dir,
        index_sha256=index_sha256,
        receipt_path=path,
        require_source_files=require_source_files,
    )
    index_file = export_dir / INDEX_FILE
    if index_file.exists():
        exported_index = _verify_audit_index_source(
            _load_json(index_file),
            str(index_file),
        )
        if exported_index != audit_index:
            raise ReceiptError(f"{path} embedded audit index differs from {index_file}")
    elif require_source_files:
        raise ReceiptError(f"{path} references missing audit index {index_file}")


def _normalize_optional_string(
    value: Any,
    label: str,
    *,
    allow_embedded_whitespace: bool = True,
) -> str | None:
    if value is None:
        return None
    if not isinstance(value, str) or not value.strip():
        raise ReceiptError(f"{label} must be null or a non-empty string")
    if any(ord(ch) < 0x20 or ord(ch) == 0x7F for ch in value):
        raise ReceiptError(f"{label} must not contain control characters")
    if value != value.strip():
        raise ReceiptError(f"{label} must not have surrounding whitespace")
    if not allow_embedded_whitespace and any(ch.isspace() for ch in value):
        raise ReceiptError(f"{label} must not contain whitespace")
    return value


def _verify_rail_sidecar(
    path: Path,
    sidecar_path: Path,
    *,
    payload_sha256: str,
    message_type: str,
    profile: str | None,
    rail_message_id: str | None,
) -> None:
    sidecar = _load_json(sidecar_path)
    if not isinstance(sidecar, dict):
        raise ReceiptError(f"{sidecar_path} must contain a JSON object")
    _reject_unknown_keys(sidecar, RAIL_SIDECAR_KEYS, str(sidecar_path))
    if sidecar.get("payload_sha256") != payload_sha256:
        raise ReceiptError(f"{path} payload_sha256 does not match source sidecar")
    if sidecar.get("message_type") != message_type:
        raise ReceiptError(f"{path} message_type does not match source sidecar")
    sidecar_profile = _normalize_optional_string(
        sidecar.get("profile"),
        f"{sidecar_path} profile",
        allow_embedded_whitespace=False,
    )
    if sidecar_profile != profile:
        raise ReceiptError(f"{path} profile does not match source sidecar")
    sidecar_rail_message_id = _normalize_optional_string(
        sidecar.get("rail_message_id"),
        f"{sidecar_path} rail_message_id",
        allow_embedded_whitespace=False,
    )
    if sidecar_rail_message_id != rail_message_id:
        raise ReceiptError(f"{path} rail_message_id does not match source sidecar")


def _verify_rail_source(
    receipt: dict[str, Any],
    path: Path,
    *,
    require_source_files: bool,
    allow_legacy_colr007: bool,
) -> None:
    payload_sha256 = receipt.get("payload_sha256")
    if not _is_lower_hex_sha256(payload_sha256):
        raise ReceiptError(f"{path} has invalid payload_sha256")
    message_type = _require_clean_string(receipt.get("message_type"), f"{path} message_type")
    if message_type in LEGACY_RAIL_MESSAGE_TYPES and not allow_legacy_colr007:
        raise ReceiptError(
            f"{path} uses legacy rail message_type {message_type!r}; "
            "production evidence must use colr.012"
        )
    profile = _normalize_optional_string(
        receipt.get("profile"),
        f"{path} profile",
        allow_embedded_whitespace=False,
    )
    rail_message_id = _normalize_optional_string(
        receipt.get("rail_message_id"),
        f"{path} rail_message_id",
        allow_embedded_whitespace=False,
    )

    xml_path_raw = _require_clean_string(receipt.get("xml_path"), f"{path} xml_path")
    xml_path = Path(xml_path_raw)
    sidecar_path_raw = _require_clean_string(
        receipt.get("sidecar_path"),
        f"{path} sidecar_path",
    )
    sidecar_path = Path(sidecar_path_raw)
    expected_sidecar = xml_path.with_suffix(xml_path.suffix + ".json")
    if sidecar_path.resolve() != expected_sidecar.resolve():
        raise ReceiptError(f"{path} sidecar_path must match xml_path sidecar")
    if sidecar_path.exists():
        _verify_rail_sidecar(
            path,
            sidecar_path,
            payload_sha256=payload_sha256,
            message_type=message_type,
            profile=profile,
            rail_message_id=rail_message_id,
        )
    elif require_source_files:
        raise ReceiptError(f"{path} references missing sidecar_path {sidecar_path}")
    if not xml_path.exists():
        if require_source_files:
            raise ReceiptError(f"{path} references missing xml_path {xml_path}")
        return

    actual = sha256_hex(_read_regular_file(xml_path))
    if actual != payload_sha256:
        raise ReceiptError(f"{path} payload_sha256 does not match source XML {xml_path}")


def verify_receipt_file(
    path: Path,
    *,
    allow_failed: bool,
    allow_insecure_http: bool,
    allow_legacy_colr007: bool,
    require_source_files: bool,
) -> dict[str, Any]:
    """Verify one operator receipt and return its parsed JSON object."""

    receipt = _load_json(path)
    if not isinstance(receipt, dict):
        raise ReceiptError(f"{path} must contain a JSON object")
    if receipt.get("version") != RECEIPT_VERSION:
        raise ReceiptError(f"{path} has unsupported receipt version")
    kind = receipt.get("receipt_kind")
    if kind not in SUPPORTED_KINDS:
        raise ReceiptError(f"{path} has unsupported receipt_kind {kind!r}")
    _reject_unknown_keys(receipt, RECEIPT_KEYS_BY_KIND[kind], str(path))
    require_digest_matches(receipt, RECEIPT_DIGEST_FIELD, str(path))
    _check_no_secret_material(receipt, path)
    _check_status(receipt, path, allow_failed=allow_failed)
    _check_response_metadata(receipt, path)

    if kind == "iso-audit-notary":
        _check_timestamp(receipt, "published_at", path)
        endpoint = _require_clean_string(receipt.get("endpoint"), f"{path} endpoint")
        _require_https(endpoint, allow_insecure_http=allow_insecure_http, label=str(path))
        _check_endpoint_digest(receipt, path, endpoint)
        _verify_anchor_source(receipt, path, require_source_files=require_source_files)
    elif kind == "iso-rail-gateway":
        _check_timestamp(receipt, "submitted_at", path)
        endpoint_url = _require_clean_string(
            receipt.get("endpoint_url"),
            f"{path} endpoint_url",
        )
        _require_https(endpoint_url, allow_insecure_http=allow_insecure_http, label=str(path))
        _check_endpoint_digest(receipt, path, endpoint_url)
        _verify_rail_source(
            receipt,
            path,
            require_source_files=require_source_files,
            allow_legacy_colr007=allow_legacy_colr007,
        )
    else:  # pragma: no cover - guarded above, kept explicit for future kinds.
        raise ReceiptError(f"{path} has unsupported receipt_kind {kind!r}")

    return receipt


def discover_receipts(receipt_dir: Path) -> list[Path]:
    """Return receipt files in deterministic order."""

    try:
        metadata = receipt_dir.lstat()
    except FileNotFoundError as error:
        raise ReceiptError(f"{receipt_dir} does not exist") from error
    if stat.S_ISLNK(metadata.st_mode):
        raise ReceiptError(f"{receipt_dir} must not be a symlink")
    if not stat.S_ISDIR(metadata.st_mode):
        raise ReceiptError(f"{receipt_dir} is not a directory")
    receipts = sorted(receipt_dir.glob("*.receipt.json"))
    if not receipts:
        raise ReceiptError(f"{receipt_dir} has no *.receipt.json files")
    return receipts


def _reject_duplicate_paths(paths: list[Path]) -> None:
    seen: dict[str, int] = {}
    for offset, path in enumerate(paths):
        key = str(path.resolve())
        if key in seen:
            raise ReceiptError(
                f"receipt[{offset}] duplicates receipt[{seen[key]}]: {key}"
            )
        seen[key] = offset


def _receipt_metadata(path: Path, receipt: dict[str, Any]) -> dict[str, Any]:
    metadata: dict[str, Any] = {
        "path": str(path),
        "receipt_kind": receipt["receipt_kind"],
        "receipt_sha256": receipt[RECEIPT_DIGEST_FIELD],
        "ok": receipt.get("ok"),
        "status_code": receipt.get("status_code"),
    }
    if receipt["receipt_kind"] == "iso-audit-notary":
        metadata.update(
            {
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
            }
        )
    return metadata


def run(args: argparse.Namespace) -> int:
    paths = list(args.receipt)
    for receipt_dir in args.receipt_dir:
        paths.extend(discover_receipts(receipt_dir))
    if not paths:
        raise ReceiptError("provide at least one --receipt or --receipt-dir")
    _reject_duplicate_paths(paths)

    verified: list[dict[str, Any]] = []
    receipt_entries: list[dict[str, Any]] = []
    seen_receipt_digests: dict[str, Path] = {}
    for path in paths:
        receipt = verify_receipt_file(
            path,
            allow_failed=args.allow_failed,
            allow_insecure_http=args.allow_insecure_http,
            allow_legacy_colr007=args.allow_legacy_colr007,
            require_source_files=args.require_source_files,
        )
        receipt_digest = receipt[RECEIPT_DIGEST_FIELD]
        if receipt_digest in seen_receipt_digests:
            raise ReceiptError(
                f"{path} {RECEIPT_DIGEST_FIELD} duplicates "
                f"{seen_receipt_digests[receipt_digest]}: {receipt_digest}"
            )
        seen_receipt_digests[receipt_digest] = path
        verified.append(receipt)
        receipt_entries.append(_receipt_metadata(path, receipt))

    summary = {
        "verified_receipts": len(verified),
        "receipt_kind": sorted({receipt["receipt_kind"] for receipt in verified}),
        "allow_failed": args.allow_failed,
        "allow_insecure_http": args.allow_insecure_http,
        "allow_legacy_colr007": args.allow_legacy_colr007,
        "require_source_files": args.require_source_files,
        "receipts": receipt_entries,
    }
    summary[SUMMARY_DIGEST_FIELD] = sha256_hex(_canonical_summary_json_bytes(summary))
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Verify ISO 20022 operator rail/notary adapter receipts."
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
        "--require-source-files",
        action="store_true",
        help="Require referenced source XML/anchor files to exist and match receipt digests.",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    try:
        return run(args)
    except ReceiptError as error:
        print(f"error: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
