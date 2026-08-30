"""Scalar and collection normalization for Torii client configuration."""

from __future__ import annotations

import math
from typing import Any, Dict, Iterable, Mapping, Optional

_HTTP_METHODS = frozenset({"DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"})


def _coerce_int(value: Any, name: str, *, allow_zero: bool = False) -> Optional[int]:
    if value is None or value == "":
        return None
    if isinstance(value, bool):
        raise TypeError(f"{name} must be an integer")
    if isinstance(value, int):
        number = value
    elif isinstance(value, str):
        digits = value[1:] if value.startswith("-") else value
        if not digits.isdecimal():
            raise TypeError(f"{name} must be an integer")
        number = int(value)
    else:
        raise TypeError(f"{name} must be an integer")
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be {'non-negative' if allow_zero else 'positive'}")
    return number


def _coerce_float(value: Any, name: str, *, allow_zero: bool = False) -> Optional[float]:
    if value is None or value == "":
        return None
    if isinstance(value, bool):
        raise TypeError(f"{name} must be numeric")
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be numeric") from None
    if not math.isfinite(number):
        raise ValueError(f"{name} must be finite")
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be greater than 0")
    return number


def _coerce_duration_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    millis = _coerce_float(value, "duration_ms", allow_zero=True)
    if millis is not None:
        return millis / 1000.0
    return _coerce_float(default_value, "duration", allow_zero=True)


def _coerce_timeout_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    millis = _coerce_float(value, "timeout_ms", allow_zero=False)
    result = None if millis is None else millis / 1000.0
    if result is not None:
        return result
    return _coerce_float(default_value, "timeout", allow_zero=False)


def _parse_retry_statuses(value: Any) -> Optional[set[int]]:
    if value is None or value == "":
        return None
    parts: Iterable[Any]
    if isinstance(value, str):
        parts = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        parts = value
    else:
        parts = [value]
    result: set[int] = set()
    for entry in parts:
        if isinstance(entry, bool) or not isinstance(entry, (int, str)):
            raise TypeError("retry_statuses entries must be integers")
        if isinstance(entry, str) and not entry.isdecimal():
            raise TypeError("retry_statuses entries must be integers")
        status = int(entry)
        if not 400 <= status <= 599:
            raise ValueError("retry_statuses entries must be HTTP error statuses (400..599)")
        result.add(status)
    return result


def _parse_retry_methods(value: Any) -> Optional[set[str]]:
    if value is None or value == "":
        return None
    entries: Iterable[Any]
    if isinstance(value, str):
        entries = [part.strip() for part in value.split(",") if part.strip()]
    elif isinstance(value, (list, tuple, set)):
        entries = value
    else:
        entries = [value]
    result: set[str] = set()
    for entry in entries:
        if not isinstance(entry, str) or not entry or entry != entry.strip():
            raise TypeError("retry_methods entries must be non-empty strings")
        method = entry.upper()
        if method not in _HTTP_METHODS:
            raise ValueError(f"retry_methods contains unsupported HTTP method {entry!r}")
        result.add(method)
    return result


def _normalize_headers(headers: Any) -> Dict[str, str]:
    if headers is None:
        return {}
    if not isinstance(headers, Mapping):
        raise TypeError("default_headers must be a mapping")
    normalized: Dict[str, str] = {}
    for key, value in headers.items():
        if not isinstance(key, str) or not key:
            raise TypeError("default_headers names must be non-empty strings")
        if not isinstance(value, str):
            raise TypeError(f"default_headers[{key!r}] must be a string")
        if any(ord(character) < 0x20 or ord(character) == 0x7F for character in value):
            raise ValueError(
                f"default_headers[{key!r}] must not contain control characters"
            )
        normalized[key] = value
    return normalized
