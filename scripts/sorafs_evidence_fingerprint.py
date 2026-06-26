"""Shared payload-free fingerprint helpers for SoraFS evidence gates."""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any


def artifact_fingerprint(
    payload: Mapping[str, Any], fields: Sequence[str]
) -> dict[str, Any]:
    """Return the selected payload-free fields from an evidence artifact."""

    return {field: payload.get(field) for field in fields}
