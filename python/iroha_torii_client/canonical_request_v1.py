"""Shared first-release limits and form encoding for canonical Torii requests."""

from __future__ import annotations

from typing import Any, Callable, Optional
from urllib.parse import parse_qsl

CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64
CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024
CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32
CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024
CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 = 36 * 1024


def canonical_query_string(raw: Optional[str]) -> str:
    """Return Torii's bounded canonical form for a raw query string."""

    if not raw:
        return ""
    raw_bytes = raw.encode("utf-8")
    if len(raw_bytes) > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1:
        raise ValueError(
            "canonical request query exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1} raw UTF-8 bytes"
        )
    pair_count = sum(1 for component in raw.split("&") if component)
    if pair_count > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1:
        raise ValueError(
            "canonical request query exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs"
        )
    pairs = parse_qsl(raw, keep_blank_values=True, strict_parsing=False)
    pairs.sort(key=lambda item: (item[0].encode("utf-8"), item[1].encode("utf-8")))
    return "&".join(
        f"{_canonical_form_encode(key)}={_canonical_form_encode(value)}"
        for key, value in pairs
    )


def validate_target(method: str, path: str) -> None:
    """Reject a method or percent-encoded path outside the V1 byte limits."""

    if len(method.encode("utf-8")) > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1:
        raise ValueError(
            "canonical request method exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} UTF-8 bytes"
        )
    if len(path.encode("utf-8")) > CANONICAL_REQUEST_MAX_PATH_BYTES_V1:
        raise ValueError(
            "canonical request path exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_PATH_BYTES_V1} UTF-8 bytes"
        )


def require_account_literal(value: Any, context: str) -> str:
    """Return an exact non-empty V1 account literal or raise an input error."""

    account = _require_exact_non_empty_string(value, context)
    if len(account.encode("utf-8")) > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1:
        raise ValueError(
            f"{context} exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} UTF-8 bytes"
        )
    return account


def account_header_value(account: str, decode_i105: Callable[[str], bytes]) -> str:
    """Return the portable ASCII V1 header spelling for an account or alias."""

    try:
        canonical = decode_i105(account)
    except ValueError:
        if not account.isascii() or any(ord(char) < 0x21 or ord(char) > 0x7E for char in account):
            raise ValueError(
                "account_id must be canonical I105 or a printable ASCII account alias"
            ) from None
        return account
    return f"0x{canonical.hex()}"


def require_nonce(value: Any, context: str) -> str:
    """Return a non-empty printable-ASCII V1 nonce or raise an input error."""

    nonce = _require_exact_non_empty_string(value, context)
    encoded = nonce.encode("utf-8")
    if len(encoded) > 256:
        raise ValueError(f"{context} must contain at most 256 ASCII bytes")
    if any(byte < 0x21 or byte > 0x7E for byte in encoded):
        raise ValueError(f"{context} must contain only printable ASCII bytes")
    return nonce


def _canonical_form_encode(value: str) -> str:
    encoded = []
    for byte in value.encode("utf-8"):
        if (
            ord("A") <= byte <= ord("Z")
            or ord("a") <= byte <= ord("z")
            or ord("0") <= byte <= ord("9")
            or byte in b"*-._"
        ):
            encoded.append(chr(byte))
        elif byte == ord(" "):
            encoded.append("+")
        else:
            encoded.append(f"%{byte:02X}")
    return "".join(encoded)


def _require_exact_non_empty_string(value: Any, context: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{context} must be a string")
    stripped = value.strip()
    if not stripped:
        raise ValueError(f"{context} must be a non-empty string")
    if stripped != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    return value
