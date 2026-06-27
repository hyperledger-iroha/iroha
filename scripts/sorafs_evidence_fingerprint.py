"""Shared payload-free fingerprint helpers for SoraFS evidence gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


def artifact_fingerprint(
    payload: Mapping[str, Any], fields: Sequence[str]
) -> dict[str, Any]:
    """Return the selected payload-free fields from an evidence artifact."""

    if not isinstance(payload, Mapping):
        raise ValueError("artifact fingerprint payload must be an object")
    if isinstance(fields, (str, bytes)) or not isinstance(fields, Sequence):
        raise ValueError("artifact fingerprint fields must be a sequence of strings")

    fingerprint: dict[str, Any] = {}
    seen_fields: set[str] = set()
    for field in fields:
        if not isinstance(field, str) or not field.strip():
            raise ValueError("artifact fingerprint fields must be non-empty strings")
        if field != field.strip():
            raise ValueError("artifact fingerprint fields must be canonical strings")
        if field in seen_fields:
            raise ValueError("artifact fingerprint fields must not contain duplicates")
        seen_fields.add(field)
        fingerprint[field] = payload.get(field)
    return fingerprint
