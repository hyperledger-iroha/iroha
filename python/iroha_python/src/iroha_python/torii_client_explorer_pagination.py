"""Strict Explorer cursor and page-limit normalization."""

from __future__ import annotations

import base64
import binascii
import re
from typing import Any, Optional


_EXPLORER_CURSOR_MAX_LENGTH = 1_424
_EXPLORER_CURSOR_PATTERN = re.compile(r"^[A-Za-z0-9_-]+$")


def _normalize_explorer_cursor(value: Any, label: str) -> Optional[str]:
    if value is None:
        return None
    if not isinstance(value, str):
        raise TypeError(f"{label} must be a string")
    if not value or len(value) > _EXPLORER_CURSOR_MAX_LENGTH:
        raise ValueError(f"{label} must contain 1..{_EXPLORER_CURSOR_MAX_LENGTH} characters")
    if _EXPLORER_CURSOR_PATTERN.fullmatch(value) is None:
        raise ValueError(f"{label} must be canonical base64url without padding")
    padding = "=" * ((4 - len(value) % 4) % 4)
    try:
        decoded = base64.b64decode(
            value + padding,
            altchars=b"-_",
            validate=True,
        )
    except (binascii.Error, ValueError) as error:
        raise ValueError(f"{label} must be canonical base64url without padding") from error
    if base64.urlsafe_b64encode(decoded).rstrip(b"=").decode("ascii") != value:
        raise ValueError(f"{label} must be canonical base64url without padding")
    return value


def _normalize_explorer_limit(value: Any, label: str) -> Optional[int]:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{label} must be an integer")
    if value < 1 or value > 100:
        raise ValueError(f"{label} must be between 1 and 100")
    return value
