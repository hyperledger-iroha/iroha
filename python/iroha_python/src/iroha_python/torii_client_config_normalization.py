"""Scalar and collection normalization for Torii client configuration."""

from __future__ import annotations

from typing import Any, Dict, Iterable, Mapping, Optional


def _coerce_int(value: Any, name: str, *, allow_zero: bool = False) -> Optional[int]:
    if value is None or value == "":
        return None
    try:
        number = int(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be an integer") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be {'non-negative' if allow_zero else 'positive'}")
    return number


def _coerce_float(value: Any, name: str, *, allow_zero: bool = False) -> Optional[float]:
    if value is None or value == "":
        return None
    try:
        number = float(value)
    except (TypeError, ValueError):
        raise TypeError(f"{name} must be numeric") from None
    if number < 0 or (number == 0 and not allow_zero):
        raise ValueError(f"{name} must be greater than 0")
    return number


def _coerce_duration_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    millis = _coerce_float(value, "duration_ms", allow_zero=True)
    if millis is not None:
        return millis / 1000.0
    return _coerce_float(default_value, "duration", allow_zero=True)


def _coerce_timeout_seconds(value: Any, *, default_value: Any = None) -> Optional[float]:
    result = _coerce_duration_seconds(value)
    if result is not None:
        return result
    return _coerce_float(default_value, "timeout", allow_zero=True)


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
    return {int(entry) for entry in parts}


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
    return {str(entry).upper() for entry in entries}


def _normalize_headers(headers: Any) -> Dict[str, str]:
    normalized: Dict[str, str] = {}
    if isinstance(headers, Mapping):
        for key, value in headers.items():
            if value is not None:
                normalized[str(key)] = str(value)
    return normalized
