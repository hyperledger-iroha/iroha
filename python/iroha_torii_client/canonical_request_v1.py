"""Shared first-release limits and form encoding for canonical Torii requests."""

from __future__ import annotations

import base64
import binascii
import re
from typing import Any, Callable, Mapping, Optional
from urllib.parse import parse_qsl

CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64
CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024
CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32
CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024
CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 = 36 * 1024
CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 = 3309
CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 = 768 * 1024

_HTTP_TOKEN = re.compile(r"[!#$%&'*+\-.^_`|~0-9A-Za-z]+")
_RAW_PATH = re.compile(r"/[!$&'()*+,\-./0-9:;=@A-Z_a-z~%]*")
_ALIAS_SEGMENT = re.compile(r"[a-z0-9_-]+")


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

    if not isinstance(method, str) or _HTTP_TOKEN.fullmatch(method) is None:
        raise ValueError("canonical request method must be a non-empty ASCII HTTP token")
    if len(method.encode("utf-8")) > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1:
        raise ValueError(
            "canonical request method exceeds the V1 limit of "
            f"{CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} UTF-8 bytes"
        )
    if (
        not isinstance(path, str)
        or path.startswith("//")
        or _RAW_PATH.fullmatch(path) is None
        or re.search(r"%(?![0-9A-Fa-f]{2})", path) is not None
        or any(
            re.sub(r"%2[eE]", ".", segment) in (".", "..")
            for segment in path.split("/")
        )
    ):
        raise ValueError(
            "canonical request path must be an exact root-relative ASCII path "
            "without query or fragment"
        )
    if len(path) > CANONICAL_REQUEST_MAX_PATH_BYTES_V1:
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
        if not is_canonical_account_alias(account):
            raise ValueError(
                "account_id must be canonical I105 or an exact ASCII account alias"
            ) from None
        return account
    return f"0x{canonical.hex()}"


def is_canonical_account_alias(value: str) -> bool:
    """Return whether ``value`` has the bounded ASCII alias header shape."""

    if (
        not isinstance(value, str)
        or value.startswith("0x")
        or not value.isascii()
        or value.count("@") != 1
    ):
        # Torii reserves the complete ``0x`` header prefix for canonical
        # address hex, even when the remaining text has alias punctuation.
        return False
    label, scope = value.split("@")
    parts = [label, *scope.split(".")]
    if len(parts) not in (2, 3):
        return False
    return all(_is_canonical_alias_segment(part) for part in parts)


def _is_canonical_alias_segment(value: str) -> bool:
    if not 1 <= len(value) <= 63 or _ALIAS_SEGMENT.fullmatch(value) is None:
        return False
    if value.startswith("-") or value.endswith("-"):
        return False
    if value[2:4] == "--" and not value.startswith("xn--"):
        return False
    # Torii owns UTS-46 validation and active-catalog resolution. The SDK only
    # rejects ambiguous or unbounded header spellings before dispatch.
    return True


def split_path_query(path: str) -> tuple[str, str]:
    """Split one exact root-relative target without accepting URL reinterpretation."""

    if not isinstance(path, str):
        raise TypeError("canonical request path must be a string")
    if not path or not path.startswith("/") or path.startswith("//") or "#" in path:
        raise ValueError("canonical request target must be root-relative and fragment-free")
    path_part, separator, query = path.partition("?")
    validate_target("GET", path_part)
    return path_part, query if separator else ""


def require_signature_bytes(value: Any, context: str) -> bytes:
    """Copy one bounded, nonzero detached V1 signature."""

    try:
        view = memoryview(value)
    except TypeError as exc:
        raise TypeError(f"{context} must return bytes") from exc
    if not 1 <= view.nbytes <= CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1:
        raise ValueError(
            f"{context} must contain 1..{CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1} bytes"
        )
    payload = bytes(view)
    if not any(payload):
        raise ValueError(f"{context} must not return an all-zero signature")
    return payload


def require_witness_header(value: Any, context: str) -> str:
    """Validate a bounded exact padded-base64 witness for pass-through."""

    if not isinstance(value, str) or not value:
        raise TypeError(f"{context} must be an exact padded-base64 string")
    if len(value) > 4 * ((CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 + 2) // 3):
        raise ValueError(f"{context} exceeds the V1 witness limit")
    try:
        decoded = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError) as exc:
        raise ValueError(f"{context} must be exact padded standard-base64") from exc
    if len(decoded) > CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1:
        raise ValueError(f"{context} exceeds the V1 witness limit")
    if base64.b64encode(decoded).decode("ascii") != value:
        raise ValueError(f"{context} must be exact padded standard-base64")
    return value


def validate_forwarded_witness_header(headers: Mapping[str, Any]) -> bool:
    """Validate an optional, uniquely cased witness in caller-supplied headers."""

    values = [value for name, value in headers.items() if name.lower() == "x-iroha-witness"]
    if len(values) > 1:
        raise ValueError("canonical request headers must contain one X-Iroha-Witness value")
    if values:
        require_witness_header(values[0], "X-Iroha-Witness")
    return bool(values)


def require_zero_retry_adapter(session: Any, url: str) -> None:
    """Require a verifiable Requests adapter and reject configured retries."""

    get_adapter = getattr(session, "get_adapter", None)
    if not callable(get_adapter):
        raise ValueError("one-shot request requires a verifiable retry policy")
    try:
        retry_total = get_adapter(url).max_retries.total
    except (AttributeError, LookupError, ValueError) as exc:
        raise ValueError("one-shot request requires a verifiable retry policy") from exc
    if retry_total is not False and retry_total != 0:
        raise ValueError("one-shot request requires adapter retries to be disabled")


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
