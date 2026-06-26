"""Shared bounded JSON loading for SoraFS evidence artifacts."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
from typing import Any

CHUNK_BYTES = 1024 * 1024


def reject_non_standard_json_constant(value: str) -> None:
    """Reject Python JSON extensions such as NaN and Infinity."""

    raise ValueError(f"non-standard JSON constant `{value}` is not allowed")


def read_evidence_bytes(path: Path, max_bytes: int) -> bytes:
    """Read bounded evidence bytes from disk."""

    chunks: list[bytes] = []
    size = 0
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(CHUNK_BYTES), b""):
            size += len(chunk)
            if size > max_bytes:
                raise ValueError(f"evidence file exceeds {max_bytes} bytes")
            chunks.append(chunk)
    return b"".join(chunks)


def decode_evidence_json(raw: bytes) -> dict[str, Any]:
    """Decode evidence bytes as a JSON object."""

    payload = json.loads(
        raw.decode("utf-8"),
        parse_constant=reject_non_standard_json_constant,
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

    try:
        return load_evidence_json_with_sha256(path, max_bytes)
    except (
        OSError,
        RuntimeError,
        UnicodeDecodeError,
        json.JSONDecodeError,
        ValueError,
    ) as error:
        errors.append(f"{path}: failed to load evidence JSON: {error}")
        return None
