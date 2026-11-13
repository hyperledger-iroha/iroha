"""Canonical decoders for Torii's HTTP error envelope.

Torii renders ``iroha_torii_shared::ErrorEnvelope { code, message, details }``
in the representation the request negotiated: one canonical Norito frame for
``Accept: application/x-norito``, Norito JSON otherwise. These decoders accept
exactly those renderings of an envelope whose optional ``details`` are absent
and raise ``ValueError`` for any other body, so each caller keeps its own error
for a body that is not such an envelope.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import struct

from norito.crc64 import crc64 as _crc64_v1

TORII_ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope"
_SCHEMA_HASH_V1 = hashlib.sha256(
    b"norito:v1:type-name\x00" + TORII_ERROR_ENVELOPE_TYPE_NAME_V1.encode("utf-8")
).digest()[:16]


def _read_compact_length_v1(payload: bytes, offset: int) -> tuple[int, int]:
    value = 0
    shift = 0
    count = 0
    while offset < len(payload) and count < 10:
        byte = payload[offset]
        offset += 1
        count += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if (count > 1 and byte == 0) or value > 0xFFFF_FFFF_FFFF_FFFF:
                raise ValueError("non-canonical compact length")
            return value, offset
        shift += 7
    raise ValueError("invalid compact length")


def _read_norito_string_v1(payload: bytes, offset: int) -> tuple[str, int]:
    length, offset = _read_compact_length_v1(payload, offset)
    end = offset + length
    if end > len(payload):
        raise ValueError("truncated Norito string")
    return payload[offset:end].decode("utf-8", errors="strict"), end


def _read_norito_field_v1(payload: bytes, offset: int) -> tuple[bytes, int]:
    length, offset = _read_compact_length_v1(payload, offset)
    end = offset + length
    if end > len(payload):
        raise ValueError("truncated error-envelope field")
    return payload[offset:end], end


def _read_norito_string_field_v1(payload: bytes, offset: int) -> tuple[str, int]:
    field, offset = _read_norito_field_v1(payload, offset)
    value, field_offset = _read_norito_string_v1(field, 0)
    if field_offset != len(field):
        raise ValueError("trailing bytes in error-envelope string field")
    return value, offset


def _require_envelope_code(code: str) -> None:
    # Torii never emits an empty code; such a body is not a typed refusal.
    if not code:
        raise ValueError("error envelope code must be non-empty")


def decode_norito_error_envelope_v1(body: bytes) -> tuple[str, str]:
    """Decode one canonical, unpadded Norito envelope frame without ``details``."""

    if len(body) < 40 or body[:6] != b"NRT0\x00\x00":
        raise ValueError("invalid Norito header")
    if not hmac.compare_digest(body[6:22], _SCHEMA_HASH_V1):
        raise ValueError("wrong error-envelope schema")
    if body[22] != 0 or body[39] != 0x02:
        raise ValueError("non-canonical error-envelope framing")
    payload_length = struct.unpack_from("<Q", body, 23)[0]
    if payload_length == 0 or len(body) != 40 + payload_length:
        raise ValueError("invalid error-envelope payload length")
    payload = body[40:]
    if struct.unpack_from("<Q", body, 31)[0] != _crc64_v1(payload):
        raise ValueError("error-envelope CRC mismatch")
    code, offset = _read_norito_string_field_v1(payload, 0)
    message, offset = _read_norito_string_field_v1(payload, offset)
    details, offset = _read_norito_field_v1(payload, offset)
    if details != b"\x00":
        raise ValueError("error details must be absent")
    if offset != len(payload):
        raise ValueError("trailing error-envelope payload")
    _require_envelope_code(code)
    return code, message


def decode_json_error_envelope_v1(body: bytes) -> tuple[str, str]:
    """Decode Torii's Norito JSON envelope without ``details``.

    The body must be exactly ``{"code":…,"message":…}`` in strict UTF-8, with
    Norito's string escaping and no insignificant whitespace.
    """

    envelope = json.loads(body.decode("utf-8", errors="strict"))
    if type(envelope) is not dict or list(envelope) != ["code", "message"]:
        raise ValueError("non-canonical JSON error envelope")
    code = envelope["code"]
    message = envelope["message"]
    if type(code) is not str or type(message) is not str:
        raise ValueError("non-canonical JSON error envelope")
    # Without ASCII escaping, json.dumps escapes strings exactly as Norito JSON
    # does: quote, backslash, the short control escapes and lowercase \u00xx.
    canonical = json.dumps(
        {"code": code, "message": message}, ensure_ascii=False, separators=(",", ":")
    ).encode("utf-8")
    if not hmac.compare_digest(canonical, body):
        raise ValueError("non-canonical JSON error envelope")
    _require_envelope_code(code)
    return code, message


def decode_typed_error_envelope_v1(content_type: str | None, body: bytes) -> tuple[str, str] | None:
    """Classify a non-success body as Torii's canonical error envelope.

    The declared media type selects the representation as the Rust client does:
    ``application/x-norito`` is Norito; an absent type, ``application/json`` or an
    ``application/*+json`` type is JSON. Any other media type, and any body that
    is not exactly one canonical envelope, returns ``None``.
    """

    media_type = None if content_type is None else content_type.split(";", 1)[0].strip().lower()
    try:
        if media_type == "application/x-norito":
            return decode_norito_error_envelope_v1(body)
        if (
            media_type is None
            or media_type == "application/json"
            or (media_type.startswith("application/") and media_type.endswith("+json"))
        ):
            return decode_json_error_envelope_v1(body)
    except (RecursionError, ValueError, struct.error):
        # Deeply nested JSON exhausts the parser's recursion; it is no envelope.
        return None
    return None


__all__ = [
    "TORII_ERROR_ENVELOPE_TYPE_NAME_V1",
    "decode_json_error_envelope_v1",
    "decode_norito_error_envelope_v1",
    "decode_typed_error_envelope_v1",
]
