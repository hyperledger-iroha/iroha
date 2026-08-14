"""Pipeline-status response and option normalization."""

from __future__ import annotations

from typing import Any, Mapping, Optional

_TRANSACTION_STATUS_SCOPES = frozenset({"local", "global"})


def _extract_pipeline_status_kind(payload: Any) -> Optional[str]:
    """Return the pipeline status ``kind`` from a Torii response, if present."""

    if not isinstance(payload, Mapping):
        return None
    status = payload.get("status")
    if not isinstance(status, Mapping):
        return None
    kind = status.get("kind")
    return kind if isinstance(kind, str) else None


def _normalize_transaction_status_scope(value: str, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    normalized = value.strip().lower()
    if not normalized:
        raise ValueError(f"{context} must be a non-empty string")
    if normalized not in _TRANSACTION_STATUS_SCOPES:
        raise ValueError(f"{context} must be one of: local, global")
    return normalized
