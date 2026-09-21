"""Bounded identity-encoded responses and exact unsigned JSON for SDK readers."""

from __future__ import annotations

import json
import re
from typing import Any, Dict, Iterable, List, Optional, Tuple

import requests


def decode_exact_json_bytes(
    body: Any,
    context: str,
    *,
    maximum_bytes: int,
) -> Any:
    """Decode duplicate-safe, unsigned integer JSON under an explicit byte cap."""

    if not isinstance(body, (bytes, bytearray, memoryview)):
        raise ValueError(f"{context} body must be bytes")
    raw = bytes(body)
    if not raw:
        raise ValueError(f"{context} returned an empty body")
    if len(raw) > maximum_bytes:
        raise ValueError(f"{context} body exceeds the {maximum_bytes}-byte limit")
    if raw.startswith(b"\xef\xbb\xbf"):
        raise ValueError(f"{context} body must not contain a UTF-8 BOM")
    try:
        text = raw.decode("utf-8", "strict")
    except UnicodeDecodeError as exc:
        raise ValueError(f"{context} body must be strict UTF-8") from exc
    if not text or text != text.strip():
        raise ValueError(f"{context} body must be exact JSON without surrounding data")

    def _object_pairs(pairs: List[Tuple[str, Any]]) -> Dict[str, Any]:
        decoded: Dict[str, Any] = {}
        for key, value in pairs:
            if key in decoded:
                raise ValueError(f"{context} body contains duplicate object key {key!r}")
            decoded[key] = value
        return decoded

    def _integer(literal: str) -> int:
        if not re.fullmatch(r"(?:0|[1-9][0-9]*)", literal):
            raise ValueError(f"{context} body contains a noncanonical unsigned integer")
        return int(literal)

    def _float(literal: str) -> float:
        raise ValueError(f"{context} body must not contain floating-point numbers: {literal}")

    def _constant(literal: str) -> Any:
        raise ValueError(f"{context} body must not contain non-finite value {literal}")

    try:
        return json.loads(
            text,
            object_pairs_hook=_object_pairs,
            parse_int=_integer,
            parse_float=_float,
            parse_constant=_constant,
        )
    except json.JSONDecodeError as exc:
        raise ValueError(f"{context} body must contain one exact JSON value") from exc


def read_bounded_identity_response(
    response: requests.Response,
    maximum_bytes: int,
    context: str,
    *,
    expected_content_type: str,
) -> bytes:
    """Read one identity-encoded response under an actual-byte ceiling."""

    try:
        content_encoding = response.headers.get("Content-Encoding")
        if content_encoding is not None and content_encoding.lower() != "identity":
            raise ValueError(f"{context} Content-Encoding must be identity")

        content_type = response.headers.get("Content-Type")
        if content_type is None or content_type.split(";", 1)[0].strip().lower() != (
            expected_content_type
        ):
            raise ValueError(
                f"{context} Content-Type must be {expected_content_type}"
            )

        declared_length: Optional[int] = None
        raw_content_length = response.headers.get("Content-Length")
        if raw_content_length is not None:
            if re.fullmatch(r"(?:0|[1-9][0-9]*)", raw_content_length) is None:
                raise ValueError(
                    f"{context} Content-Length must be a canonical unsigned decimal integer"
                )
            declared_length = int(raw_content_length)
            if declared_length > maximum_bytes:
                raise ValueError(f"{context} response exceeds its byte limit")

        if isinstance(getattr(response, "_content", False), bytes):
            raise ValueError(f"{context} transport prebuffered the response body")

        output = bytearray()
        for chunk in response.iter_content(chunk_size=8_192, decode_unicode=False):
            if not chunk:
                continue
            if not isinstance(chunk, (bytes, bytearray)):
                raise TypeError(f"{context} response yielded a non-byte chunk")
            if len(chunk) > maximum_bytes - len(output):
                raise ValueError(f"{context} response exceeds its byte limit")
            output.extend(chunk)
        body = bytes(output)

        if declared_length is not None and declared_length != len(body):
            raise ValueError(f"{context} response length did not match Content-Length")
        return body
    finally:
        response.close()


def expect_status_without_body(
    response: requests.Response,
    expected: Iterable[int],
    context: str,
) -> None:
    """Reject unexpected statuses without reading or rendering response bytes."""

    expected_set = set(expected)
    if response.status_code in expected_set:
        return
    response.close()
    raise RuntimeError(
        f"{context} returned unexpected status {response.status_code}; "
        f"expected {sorted(expected_set)}"
    )
