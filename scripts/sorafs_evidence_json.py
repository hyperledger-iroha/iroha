"""Shared bounded JSON loading for SoraFS evidence artifacts."""

from __future__ import annotations

import hashlib
import json
import os
from html import unescape
from pathlib import Path
from typing import Any
from urllib.parse import unquote

from sorafs_evidence_sensitivity import (
    COMMON_SENSITIVE_KEYS,
    COMMON_SENSITIVE_KEY_NORMALIZED,
    HIGH_RISK_SENSITIVE_KEY_FRAGMENTS,
    MAX_SENSITIVE_KEY_DECODE_PASSES,
    PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES,
    normalize_sensitive_key,
)
from sorafs_evidence_paths import (
    EVIDENCE_FILE_INSPECTION_DIAGNOSTIC,
    EVIDENCE_FILE_MISSING_DIAGNOSTIC,
    EVIDENCE_FILE_SYMLINK_DIAGNOSTIC,
    validate_evidence_parent_chain,
)
from sorafs_path_identity import diagnostic_text_is_canonical, error_diagnostic_label

CHUNK_BYTES = 1024 * 1024
EVIDENCE_JSON_LOAD_DIAGNOSTIC = "failed to load evidence JSON"
EVIDENCE_JSON_READ_DIAGNOSTIC = "evidence JSON cannot be read"
JSON_DUPLICATE_KEY_EXTRA_SENSITIVE_KEYS = frozenset(
    {
        "access_log_entries",
        "account_private_key",
        "account_id",
        "audit_log_entries",
        "billing_statement",
        "body",
        "canonical_account",
        "challenge",
        "challenge_bytes",
        "credential",
        "credential_body",
        "credential_bytes",
        "credential_payload",
        "customer_email",
        "dag_block",
        "dag_head",
        "drand_randomness",
        "drand_signature",
        "evidence_json",
        "fetch_transcript",
        "gateway_private_key",
        "head_bytes",
        "holder_identity",
        "honey_response",
        "honey_responses",
        "identity_document",
        "leaf_merkle_path",
        "manifest_signing_key",
        "mnemonic",
        "nonce",
        "norito_bytes",
        "payload",
        "payload_b64",
        "payload_body",
        "payload_bytes",
        "raw",
        "raw_archive",
        "raw_body",
        "raw_evidence",
        "raw_payload",
        "raw_request",
        "raw_response",
        "response_bodies",
        "segment_merkle_path",
        "signature_key",
        "signed_transaction",
        "signed_url",
        "signed_urls",
        "snapshot_b64",
        "snapshot_bytes",
        "token_b64",
        "url_signature",
        "vrf_output",
        "watermark_key",
        "watermark_secret",
        "webauthn_assertion",
    }
)
JSON_DUPLICATE_KEY_SENSITIVE_NORMALIZED_MARKERS = frozenset(
    {
        "body",
        "credential",
        "ledger",
        "payload",
        "private",
        "proof",
        "receipt",
    }
)
JSON_DUPLICATE_KEY_PAYLOAD_FREE_REFERENCE_SUFFIXES = (
    PAYLOAD_FREE_SENSITIVE_REFERENCE_SUFFIXES | frozenset({"blake3hex", "sha256hex"})
)


class EvidenceFileTooLargeError(ValueError):
    """Raised when bounded evidence bytes exceed the configured limit."""

    def __init__(self, max_bytes: int):
        self.max_bytes = max_bytes
        super().__init__(f"evidence file exceeds {max_bytes} bytes")


def _require_error_list(errors: Any) -> list[str]:
    if not isinstance(errors, list):
        raise ValueError("evidence JSON errors must be a list of strings")
    for error in errors:
        if not isinstance(error, str):
            raise ValueError("evidence JSON errors must be a list of strings")
        if not diagnostic_text_is_canonical(error):
            raise ValueError(
                "evidence JSON errors must contain non-empty canonical strings"
            )
    return errors


def reject_non_standard_json_constant(value: str) -> None:
    """Reject Python JSON extensions such as NaN and Infinity."""

    raise ValueError(f"non-standard JSON constant `{value}` is not allowed")


def _json_key_label(key: Any) -> str:
    if isinstance(key, str) and diagnostic_text_is_canonical(key):
        if _json_key_is_sensitive(key):
            return "`<sensitive-key>`"
        return f"`{key}`"
    return "`<non-canonical>`"


def _decoded_json_key_variants(key: str) -> tuple[str, ...]:
    variants = [key]
    seen = {key}
    current = key
    for _ in range(MAX_SENSITIVE_KEY_DECODE_PASSES):
        decoded = unescape(unquote(current))
        if decoded == current or decoded in seen:
            break
        variants.append(decoded)
        seen.add(decoded)
        current = decoded
    return tuple(variants)


def _is_payload_free_sensitive_reference(normalized_key: str) -> bool:
    return any(
        normalized_key.endswith(suffix)
        for suffix in JSON_DUPLICATE_KEY_PAYLOAD_FREE_REFERENCE_SUFFIXES
    )


def _json_key_is_sensitive(key: str) -> bool:
    exact_sensitive_keys = COMMON_SENSITIVE_KEYS | JSON_DUPLICATE_KEY_EXTRA_SENSITIVE_KEYS
    normalized_sensitive_keys = COMMON_SENSITIVE_KEY_NORMALIZED | frozenset(
        normalize_sensitive_key(key_name) for key_name in exact_sensitive_keys
    )
    for variant in _decoded_json_key_variants(key):
        normalized_key = normalize_sensitive_key(variant)
        if (
            variant.lower() in exact_sensitive_keys
            or normalized_key in normalized_sensitive_keys
            or (
                normalized_key.startswith("raw")
                and not _is_payload_free_sensitive_reference(normalized_key)
            )
            or any(
                fragment in normalized_key
                and not _is_payload_free_sensitive_reference(normalized_key)
                for fragment in HIGH_RISK_SENSITIVE_KEY_FRAGMENTS
            )
            or any(
                marker in normalized_key
                and not _is_payload_free_sensitive_reference(normalized_key)
                for marker in JSON_DUPLICATE_KEY_SENSITIVE_NORMALIZED_MARKERS
            )
        ):
            return True
    return False


def _error_label(
    error: BaseException,
    *,
    path: Any = None,
    path_label: str | None = None,
) -> str:
    if isinstance(path, Path):
        return error_diagnostic_label(error, path_label=path_label)
    return error_diagnostic_label(error)


def json_object_without_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    """Build a JSON object while rejecting duplicate keys."""

    payload: dict[str, Any] = {}
    for key, value in pairs:
        if key in payload:
            raise ValueError(
                f"evidence JSON object contains duplicate key {_json_key_label(key)}"
            )
        payload[key] = value
    return payload


def validate_evidence_file_for_read(path: Path) -> None:
    """Reject unsafe evidence file paths before opening them."""

    try:
        if path.is_symlink():
            raise ValueError(EVIDENCE_FILE_SYMLINK_DIAGNOSTIC)
        parent_errors: list[str] = []
        if not validate_evidence_parent_chain(
            path,
            parent_errors,
            label="evidence file",
        ):
            raise ValueError(parent_errors[0])
        if not path.is_file():
            raise ValueError(EVIDENCE_FILE_MISSING_DIAGNOSTIC)
    except ValueError:
        raise
    except (OSError, RuntimeError) as error:
        del error
        raise RuntimeError(EVIDENCE_FILE_INSPECTION_DIAGNOSTIC) from None


def evidence_read_open_flags() -> int:
    """Return platform read flags that refuse a final symlink when available."""

    return (
        os.O_RDONLY
        | getattr(os, "O_BINARY", 0)
        | getattr(os, "O_CLOEXEC", 0)
        | getattr(os, "O_NOFOLLOW", 0)
    )


def read_evidence_bytes(path: Path, max_bytes: int) -> bytes:
    """Read bounded evidence bytes from disk."""

    if not isinstance(path, Path):
        raise ValueError("evidence path must be a path")
    if (
        not isinstance(max_bytes, int)
        or isinstance(max_bytes, bool)
        or max_bytes <= 0
    ):
        raise ValueError("evidence byte limit must be positive")
    validate_evidence_file_for_read(path)
    chunks: list[bytes] = []
    size = 0
    fd = os.open(path, evidence_read_open_flags())
    try:
        handle = os.fdopen(fd, "rb")
        fd = -1
        with handle:
            for chunk in iter(lambda: handle.read(CHUNK_BYTES), b""):
                size += len(chunk)
                if size > max_bytes:
                    raise EvidenceFileTooLargeError(max_bytes)
                chunks.append(chunk)
    finally:
        if fd >= 0:
            os.close(fd)
    return b"".join(chunks)


def decode_evidence_json(raw: bytes) -> dict[str, Any]:
    """Decode evidence bytes as a JSON object."""

    if not isinstance(raw, (bytes, bytearray)):
        raise ValueError("evidence JSON bytes must be bytes")
    payload = json.loads(
        raw.decode("utf-8"),
        parse_constant=reject_non_standard_json_constant,
        object_pairs_hook=json_object_without_duplicate_keys,
    )
    if not isinstance(payload, dict):
        raise ValueError("evidence root must be a JSON object")
    return payload


def load_evidence_json(path: Path, max_bytes: int) -> dict[str, Any]:
    """Load a bounded evidence JSON object from disk."""

    return decode_evidence_json(read_evidence_bytes(path, max_bytes))


def load_evidence_json_with_sha256(
    path: Path, max_bytes: int
) -> tuple[dict[str, Any], str]:
    """Load a bounded evidence JSON object and digest the same bytes."""

    raw = read_evidence_bytes(path, max_bytes)
    return decode_evidence_json(raw), hashlib.sha256(raw).hexdigest()


def load_evidence_json_with_sha256_or_record_error(
    path: Path,
    max_bytes: int,
    errors: list[str],
) -> tuple[dict[str, Any], str] | None:
    """Load evidence JSON and append the standard sanitized error on failure."""

    error_list = _require_error_list(errors)
    try:
        return load_evidence_json_with_sha256(path, max_bytes)
    except (OSError, RuntimeError):
        error_list.append(f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: {EVIDENCE_JSON_READ_DIAGNOSTIC}")
        return None
    except (UnicodeDecodeError, json.JSONDecodeError, ValueError) as error:
        error_label = _error_label(error)
        error_list.append(f"{EVIDENCE_JSON_LOAD_DIAGNOSTIC}: {error_label}")
        return None
