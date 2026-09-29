"""Strict Ed25519 verification and canonical JSON helpers for release evidence.

Release operators provide detached Ed25519 signatures made outside the
repository; the SoraFS release and qualification tools only verify those
signatures and the canonical public JSON they cover.
"""

from __future__ import annotations

import hashlib
import json
from typing import Any


MAX_JSON_DEPTH = 32
MAX_JSON_NODES = 32_768


class ReleaseEvidenceError(ValueError):
    """A bounded, public-safe release-evidence validation failure."""


def _fail(message: str) -> None:
    raise ReleaseEvidenceError(message)


def _reject_duplicate_keys(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if type(key) is not str or key in result:
            _fail("JSON contains a duplicate or non-string object key")
        result[key] = value
    return result


def _reject_json_constant(value: str) -> None:
    _fail(f"JSON contains forbidden non-finite number {value}")


def _json_shape(value: Any, *, depth: int = 0) -> int:
    if depth > MAX_JSON_DEPTH:
        _fail("JSON nesting exceeds the release-evidence limit")
    if value is None or type(value) in (bool, int, str):
        return 1
    if type(value) is list:
        total = 1
        for item in value:
            total += _json_shape(item, depth=depth + 1)
            if total > MAX_JSON_NODES:
                _fail("JSON node count exceeds the release-evidence limit")
        return total
    if type(value) is dict:
        total = 1
        for key, item in value.items():
            if type(key) is not str:
                _fail("JSON object keys must be strings")
            total += 1 + _json_shape(item, depth=depth + 1)
            if total > MAX_JSON_NODES:
                _fail("JSON node count exceeds the release-evidence limit")
        return total
    _fail("JSON contains a value outside the canonical subset")


def parse_json_bytes(data: bytes, *, label: str, maximum: int) -> Any:
    """Decode strict UTF-8 JSON with duplicate-key and shape limits."""

    if type(data) is not bytes or not data or len(data) > maximum:
        _fail(f"{label} must contain between 1 and {maximum} bytes")
    if data.startswith(b"\xef\xbb\xbf") or b"\x00" in data:
        _fail(f"{label} must be canonical UTF-8 JSON without BOM or NUL")
    try:
        text = data.decode("utf-8", "strict")
    except UnicodeDecodeError:
        _fail(f"{label} is not valid UTF-8")
    try:
        value = json.loads(
            text,
            object_pairs_hook=_reject_duplicate_keys,
            parse_constant=_reject_json_constant,
        )
    except ReleaseEvidenceError:
        raise
    except (json.JSONDecodeError, RecursionError, ValueError):
        _fail(f"{label} is not valid canonical JSON")
    _json_shape(value)
    return value


def canonical_json_bytes(value: Any) -> bytes:
    """Return the single canonical JSON encoding used by release-evidence tools."""

    _json_shape(value)
    try:
        return json.dumps(
            value,
            ensure_ascii=True,
            allow_nan=False,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("ascii")
    except (TypeError, ValueError, RecursionError):
        _fail("value cannot be encoded as canonical JSON")


def canonical_json_file_bytes(value: Any) -> bytes:
    """Return canonical JSON followed by exactly one LF."""

    return canonical_json_bytes(value) + b"\n"


def require_canonical_json_file(data: bytes, value: Any, *, label: str) -> None:
    if data != canonical_json_file_bytes(value):
        _fail(f"{label} must use canonical sorted compact JSON and one trailing LF")


_ED_Q = 2**255 - 19
_ED_L = 2**252 + 27742317777372353535851937790883648493
_ED_D = (-121665 * pow(121666, _ED_Q - 2, _ED_Q)) % _ED_Q
_ED_I = pow(2, (_ED_Q - 1) // 4, _ED_Q)
_ED_IDENTITY = (0, 1)


def _ed_xrecover(y: int) -> int | None:
    xx = (y * y - 1) * pow(_ED_D * y * y + 1, _ED_Q - 2, _ED_Q) % _ED_Q
    x = pow(xx, (_ED_Q + 3) // 8, _ED_Q)
    if (x * x - xx) % _ED_Q != 0:
        x = x * _ED_I % _ED_Q
    if (x * x - xx) % _ED_Q != 0:
        return None
    return x


def _ed_decode(encoded: bytes) -> tuple[int, int] | None:
    if len(encoded) != 32:
        return None
    raw = int.from_bytes(encoded, "little")
    sign_bit = raw >> 255
    y = raw & ((1 << 255) - 1)
    if y >= _ED_Q:
        return None
    x = _ed_xrecover(y)
    if x is None:
        return None
    if (x & 1) != sign_bit:
        x = (-x) % _ED_Q
    if x == 0 and sign_bit:
        return None
    point = (x, y)
    if _ed_encode(point) != encoded:
        return None
    return point


def _ed_encode(point: tuple[int, int]) -> bytes:
    x, y = point
    return (y | ((x & 1) << 255)).to_bytes(32, "little")


def _ed_extended(point: tuple[int, int]) -> tuple[int, int, int, int]:
    x, y = point
    return x, y, 1, x * y % _ED_Q


_ED_EXTENDED_IDENTITY = (0, 1, 1, 0)


def _ed_add_extended(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> tuple[int, int, int, int]:
    x1, y1, z1, t1 = left
    x2, y2, z2, t2 = right
    a = (y1 - x1) * (y2 - x2) % _ED_Q
    b = (y1 + x1) * (y2 + x2) % _ED_Q
    c = 2 * _ED_D * t1 * t2 % _ED_Q
    d = 2 * z1 * z2 % _ED_Q
    e = (b - a) % _ED_Q
    f = (d - c) % _ED_Q
    g = (d + c) % _ED_Q
    h = (b + a) % _ED_Q
    return e * f % _ED_Q, g * h % _ED_Q, f * g % _ED_Q, e * h % _ED_Q


def _ed_scalar_multiply_extended(
    point: tuple[int, int, int, int], scalar: int
) -> tuple[int, int, int, int]:
    result = _ED_EXTENDED_IDENTITY
    addend = point
    value = scalar
    while value:
        if value & 1:
            result = _ed_add_extended(result, addend)
        addend = _ed_add_extended(addend, addend)
        value >>= 1
    return result


def _ed_extended_equal(
    left: tuple[int, int, int, int], right: tuple[int, int, int, int]
) -> bool:
    return (left[0] * right[2] - right[0] * left[2]) % _ED_Q == 0 and (
        left[1] * right[2] - right[1] * left[2]
    ) % _ED_Q == 0


def _ed_extended_to_affine(point: tuple[int, int, int, int]) -> tuple[int, int]:
    inverse = pow(point[2], _ED_Q - 2, _ED_Q)
    return point[0] * inverse % _ED_Q, point[1] * inverse % _ED_Q


def _ed_scalar_multiply(point: tuple[int, int], scalar: int) -> tuple[int, int]:
    return _ed_extended_to_affine(
        _ed_scalar_multiply_extended(_ed_extended(point), scalar)
    )


_ED_BASE_Y = 4 * pow(5, _ED_Q - 2, _ED_Q) % _ED_Q
_ED_BASE_X = _ed_xrecover(_ED_BASE_Y)
assert _ED_BASE_X is not None
if _ED_BASE_X & 1:
    _ED_BASE_X = _ED_Q - _ED_BASE_X
_ED_BASE = (_ED_BASE_X, _ED_BASE_Y)


def verify_ed25519(public_key: bytes, signature: bytes, message: bytes) -> bool:
    """Verify a strict, canonical, prime-subgroup Ed25519 signature."""

    if len(public_key) != 32 or len(signature) != 64:
        return False
    public_point = _ed_decode(public_key)
    r_point = _ed_decode(signature[:32])
    scalar = int.from_bytes(signature[32:], "little")
    if public_point is None or r_point is None or scalar >= _ED_L:
        return False
    if public_point == _ED_IDENTITY or r_point == _ED_IDENTITY:
        return False
    public_extended = _ed_extended(public_point)
    r_extended = _ed_extended(r_point)
    if not _ed_extended_equal(
        _ed_scalar_multiply_extended(public_extended, _ED_L),
        _ED_EXTENDED_IDENTITY,
    ):
        return False
    if not _ed_extended_equal(
        _ed_scalar_multiply_extended(r_extended, _ED_L),
        _ED_EXTENDED_IDENTITY,
    ):
        return False
    challenge = (
        int.from_bytes(
            hashlib.sha512(signature[:32] + public_key + message).digest(), "little"
        )
        % _ED_L
    )
    return _ed_extended_equal(
        _ed_scalar_multiply_extended(_ed_extended(_ED_BASE), scalar),
        _ed_add_extended(
            r_extended,
            _ed_scalar_multiply_extended(public_extended, challenge),
        ),
    )
