"""Shared fail-closed validators for public SDK inputs."""

from __future__ import annotations

import math
import re
from typing import Any, Dict, Mapping, Optional


def _optional_uint(
    value: Any,
    context: str,
    *,
    maximum: int,
    allow_zero: bool,
) -> Optional[int]:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{context} must be an integer when provided")
    minimum = 0 if allow_zero else 1
    if value < minimum or value > maximum:
        qualifier = "non-negative" if allow_zero else "positive"
        raise ValueError(f"{context} must be a {qualifier} integer no greater than {maximum}")
    return value


def _normalize_json_value(value: Any, context: str) -> Any:
    """Copy one finite JSON value without lossy string coercion."""

    if value is None or isinstance(value, (bool, str, int)):
        return value
    if isinstance(value, float):
        if not math.isfinite(value):
            raise ValueError(f"{context} must not contain NaN or Infinity")
        return value
    if isinstance(value, Mapping):
        normalized: Dict[str, Any] = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError(f"{context} keys must be strings")
            normalized[key] = _normalize_json_value(item, f"{context}.{key}")
        return normalized
    if isinstance(value, (list, tuple)):
        return [
            _normalize_json_value(item, f"{context}[{index}]") for index, item in enumerate(value)
        ]
    raise TypeError(f"{context} must contain only exact JSON values")


def _normalize_mapping_payload(payload: Mapping[str, Any], context: str) -> Dict[str, Any]:
    if not isinstance(payload, Mapping):
        raise TypeError(f"{context} must be a mapping")
    normalized = _normalize_json_value(payload, context)
    if not isinstance(normalized, dict):  # pragma: no cover - mapping contract
        raise TypeError(f"{context} must serialize to a JSON object")
    return normalized


def _validate_state_row_selectors(rows: list[dict[str, str]]) -> None:
    """Bound originals before allocation and admit the canonical sorted (table, key) order."""
    if type(rows) is not list or not 1 <= len(rows) <= 32:
        raise ValueError("expected between one and 32 State selectors")
    previous = None
    for row in rows:
        if type(row) is not dict or row.keys() != {"table", "key_hex"}:
            raise ValueError("expected exactly table and key_hex for each State selector")
        table, key = row["table"], row["key_hex"]
        if (type(table) is not str or len(table) > 256 or not re.fullmatch(r"[A-Za-z0-9_.]+", table)
                or type(key) is not str or len(key) > 8192 or len(key) % 2 or not re.fullmatch(r"[0-9a-f]*", key)):
            raise ValueError("expected bounded canonical table and key_hex for each State selector")
        position = (table, key)
        if previous is not None and position <= previous:
            raise ValueError("State selectors must be sorted and distinct")
        previous = position
