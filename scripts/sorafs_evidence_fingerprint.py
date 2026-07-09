"""Shared payload-free fingerprint helpers for SoraFS evidence gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from sorafs_path_identity import diagnostic_text_is_canonical


def artifact_fingerprint(
    payload: Mapping[str, Any], fields: Sequence[str]
) -> dict[str, Any]:
    """Return the selected payload-free fields from an evidence artifact."""

    if not isinstance(payload, Mapping):
        raise ValueError("artifact fingerprint payload must be an object")
    if isinstance(fields, (str, bytes, bytearray)) or not isinstance(fields, Sequence):
        raise ValueError("artifact fingerprint fields must be a sequence of strings")

    fingerprint: dict[str, Any] = {}
    seen_fields: set[str] = set()
    for field in fields:
        if not diagnostic_text_is_canonical(field):
            raise ValueError(
                "artifact fingerprint fields must be non-empty canonical strings "
                "without control characters"
            )
        if field in seen_fields:
            raise ValueError("artifact fingerprint fields must not contain duplicates")
        seen_fields.add(field)
        value = payload.get(field)
        if value is not None:
            fingerprint[field] = value
    return fingerprint
