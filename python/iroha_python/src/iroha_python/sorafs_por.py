"""Canonical pagination helpers for SoraFS proof-of-retrievability routes."""

from __future__ import annotations

import base64
import binascii
import re
from typing import Any


_CURSOR_MAX_LENGTH = 256


def normalize_cursor(value: Any, context: str) -> str:
    """Require canonical unpadded base64url within the route cursor bound."""

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a canonical base64url string")
    if not value or len(value) > _CURSOR_MAX_LENGTH:
        raise ValueError(f"{context} must be 1..={_CURSOR_MAX_LENGTH} characters")
    if len(value) % 4 == 1 or re.fullmatch(r"[A-Za-z0-9_-]+", value) is None:
        raise ValueError(f"{context} must be canonical base64url without padding")
    padding = "=" * ((4 - len(value) % 4) % 4)
    try:
        decoded = base64.urlsafe_b64decode(value + padding)
    except (binascii.Error, ValueError) as exc:
        raise ValueError(
            f"{context} must be canonical base64url without padding"
        ) from exc
    canonical = base64.urlsafe_b64encode(decoded).decode("ascii").rstrip("=")
    if canonical != value:
        raise ValueError(f"{context} must be canonical base64url without padding")
    return value
