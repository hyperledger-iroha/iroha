"""Shared bounded JSON loading for SoraFS evidence artifacts."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
from typing import Any

from sorafs_evidence_paths import validate_evidence_parent_chain
from sorafs_path_identity import error_diagnostic_label, path_diagnostic_label

CHUNK_BYTES = 1024 * 1024


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
        if (
            not error.strip()
            or error != error.strip()
            or any(ord(character) < 32 or ord(character) == 127 for character in error)
        ):
            raise ValueError(
                "evidence JSON errors must contain non-empty canonical strings"
            )
    return errors


def reject_non_standard_json_constant(value: str) -> None:
    """Reject Python JSON extensions such as NaN and Infinity."""

    raise ValueError(f"non-standard JSON constant `{value}` is not allowed")


def _json_key_label(key: Any) -> str:
    if (
        isinstance(key, str)
        and key.strip()
        and key == key.strip()
        and not any(ord(character) < 32 or ord(character) == 127 for character in key)
    ):
        return f"`{key}`"
    return "`<non-canonical>`"


def _evidence_path_label(path: Any) -> str:
    return path_diagnostic_label(path)


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

    path_label = _evidence_path_label(path)
    try:
        if path.is_symlink():
            raise ValueError(f"evidence file `{path_label}` must not be a symlink")
        parent_errors: list[str] = []
        if not validate_evidence_parent_chain(
            path,
            parent_errors,
            label="evidence file",
        ):
            raise ValueError(parent_errors[0])
        if not path.is_file():
            raise ValueError(
                f"evidence file `{path_label}` must exist and be a file"
            )
    except ValueError:
        raise
    except (OSError, RuntimeError) as error:
        raise RuntimeError(
            f"evidence file `{path_label}` cannot be inspected: "
            f"{_error_label(error, path=path, path_label=path_label)}"
        ) from error


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
    """Load evidence JSON and append the standard path-qualified error on failure."""

    error_list = _require_error_list(errors)
    try:
        return load_evidence_json_with_sha256(path, max_bytes)
    except (
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        path_label = _evidence_path_label(path)
        error_label = _error_label(error, path=path, path_label=path_label)
        error_list.append(
            f"{path_label}: failed to load evidence JSON: {error_label}"
        )
        return None
