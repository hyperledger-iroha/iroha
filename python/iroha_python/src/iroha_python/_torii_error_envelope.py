"""Canonical decoders for Torii's HTTP error envelope.

Torii renders ``iroha_torii_shared::ErrorEnvelope { code, message, details }``
in the representation the request negotiated: one canonical Norito frame for
``Accept: application/x-norito``, Norito JSON otherwise. These decoders accept
exactly those renderings, byte for byte, of an envelope whose optional
``details`` are absent or carry only the flat fields Torii attaches to
admission, throttling and validation failures: the reject code, queue
snapshot, retry delay, AXT and fee metadata, and the textual diagnostics
(``layer``, ``endpoint``, ``field``, ``expected``, ``actual``, ``profile``,
``chain_discriminant``, ``entrypoint_hash``, ``tx_hash``, ``last_status``,
``hint``). They raise ``ValueError`` for any other body, so each caller keeps
its own error for a body that is not such an envelope.

Details that carry one of the structured observations (an IVM fault, a
pipeline-status, alias, SNS, asset or settlement-receipt absence, a
finality-attestation or AMX approval-read failure, or an alias setup report)
are outside that subset and are refused rather than partially decoded.
TODO: decode those observations here once an SDK route needs them typed; each
needs its own canonical codec.

A decoded envelope is ``(code, message, details)``. ``details`` is ``None``
when absent and otherwise a ``dict`` in the Norito JSON shape: snake_case keys,
absent options omitted and unsigned integers as ``int``.
"""

from __future__ import annotations

import hashlib
import hmac
import json
import struct
from typing import Any, Dict, Optional, Tuple

from norito.crc64 import crc64 as _crc64_v1

TORII_ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope"
_SCHEMA_HASH_V1 = hashlib.sha256(
    b"norito:v1:type-name\x00" + TORII_ERROR_ENVELOPE_TYPE_NAME_V1.encode("utf-8")
).digest()[:16]

#: ``(code, message, details)`` of one decoded envelope.
ToriiErrorEnvelopeV1 = Tuple[str, str, Optional[Dict[str, Any]]]

_TEXT = ("text",)
_BOOL = ("bool",)
_U16 = ("uint", 2)
_U32 = ("uint", 4)
_U64 = ("uint", 8)
# A structured observation whose payload schema these decoders do not decode.
_UNDECODED = ("undecoded",)


def _required(name: str, kind: tuple) -> tuple:
    return (name, kind, False)


def _optional(name: str, kind: tuple) -> tuple:
    # ``Option<T>`` with ``skip_serializing_if = "Option::is_none"``: a tagged
    # Norito field, and a JSON key that is omitted when absent.
    return (name, kind, True)


def _record(*fields: tuple) -> tuple:
    return ("record", fields)


# Each record lists the Rust struct's fields in declaration order.
_QUEUE_ERROR_SNAPSHOT_V1 = _record(
    _required("state", _TEXT),
    _required("queued", _U64),
    _required("capacity", _U64),
    _required("saturated", _BOOL),
)
_AXT_ERROR_DETAILS_V1 = _record(
    _optional("code", _TEXT),
    _optional("reason", _TEXT),
    _optional("snapshot_version", _U64),
    _optional("dataspace", _U64),
    _optional("lane", _U32),
    _optional("active_handle_era", _U64),
    _optional("next_handle_counter", _U64),
)
_FEE_ERROR_DETAILS_V1 = _record(
    _required("code", _TEXT),
    _required("retryable", _BOOL),
    _optional("program_id", _TEXT),
    _optional("program_revision", _U64),
    _optional("asset_definition_id", _TEXT),
    _optional("required", _TEXT),
    _optional("available", _TEXT),
    _optional("rule_id", _TEXT),
    _optional("observation_height", _U64),
    _optional("remediation", _TEXT),
)
_ERROR_DETAILS_V1 = _record(
    _optional("ivm_fault", _UNDECODED),
    _optional("layer", _TEXT),
    _optional("reject_code", _TEXT),
    _optional("queue", _QUEUE_ERROR_SNAPSHOT_V1),
    _optional("retry_after_seconds", _U64),
    _optional("endpoint", _TEXT),
    _optional("field", _TEXT),
    _optional("expected", _TEXT),
    _optional("actual", _TEXT),
    _optional("profile", _TEXT),
    _optional("chain_discriminant", _U16),
    _optional("entrypoint_hash", _TEXT),
    _optional("tx_hash", _TEXT),
    _optional("pipeline_transaction_status_not_found", _UNDECODED),
    _optional("finality_attestation_failure", _UNDECODED),
    _optional("amx_approval_read_failure", _UNDECODED),
    _optional("last_status", _TEXT),
    _optional("hint", _TEXT),
    _optional("axt", _AXT_ERROR_DETAILS_V1),
    _optional("fee", _FEE_ERROR_DETAILS_V1),
    _optional("alias_setup_report", _UNDECODED),
    _optional("account_alias_not_found", _UNDECODED),
    _optional("account_aliases_by_account_not_found", _UNDECODED),
    _optional("sns_registration_not_found", _UNDECODED),
    _optional("query_asset_not_found", _UNDECODED),
    _optional("query_settlement_receipt_not_found", _UNDECODED),
)
_ERROR_ENVELOPE_V1 = _record(
    _required("code", _TEXT),
    _required("message", _TEXT),
    _optional("details", _ERROR_DETAILS_V1),
)


def _undecoded_detail() -> ValueError:
    return ValueError("error details carry an observation these decoders do not decode")


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


def _read_norito_span_v1(payload: bytes, offset: int, what: str) -> tuple[bytes, int]:
    """One compact-length-prefixed span: a field, an option payload or a string."""

    length, offset = _read_compact_length_v1(payload, offset)
    end = offset + length
    if end > len(payload):
        raise ValueError(f"truncated error-envelope {what}")
    return payload[offset:end], end


def _decode_norito_value_v1(kind: tuple, data: bytes) -> Any:
    """Decode a value that must consume exactly ``data``, its declared span."""

    tag = kind[0]
    if tag == "text":
        text, offset = _read_norito_span_v1(data, 0, "string")
        if offset != len(data):
            raise ValueError("trailing bytes in error-envelope string field")
        return text.decode("utf-8", errors="strict")
    if tag == "bool":
        if len(data) != 1 or data[0] > 1:
            raise ValueError("invalid error-envelope bool")
        return data[0] == 1
    if tag == "uint":
        if len(data) != kind[1]:
            raise ValueError("invalid error-envelope integer width")
        return int.from_bytes(data, "little")
    if tag == "record":
        return _decode_norito_record_v1(kind, data)
    raise _undecoded_detail()


def _decode_norito_record_v1(kind: tuple, payload: bytes) -> Dict[str, Any]:
    """Every declared field as one length-prefixed span, in order, with no tail.

    An absent option is the lone tag 0; a present one is tag 1 followed by its
    value's length-prefixed payload.
    """

    offset = 0
    value: Dict[str, Any] = {}
    for name, field_kind, optional in kind[1]:
        span, offset = _read_norito_span_v1(payload, offset, "field")
        if not optional:
            value[name] = _decode_norito_value_v1(field_kind, span)
            continue
        if span == b"\x00":
            continue
        if not span or span[0] != 1:
            raise ValueError("invalid error-envelope option tag")
        present, inner = _read_norito_span_v1(span, 1, "option payload")
        if inner != len(span):
            raise ValueError("trailing bytes in error-envelope option field")
        value[name] = _decode_norito_value_v1(field_kind, present)
    if offset != len(payload):
        raise ValueError("trailing error-envelope payload")
    return value


def _non_canonical_json() -> ValueError:
    return ValueError("non-canonical JSON error envelope")


def _decode_json_value_v1(kind: tuple, value: Any) -> Any:
    tag = kind[0]
    if tag == "text":
        if type(value) is not str:
            raise _non_canonical_json()
        return value
    if tag == "bool":
        if type(value) is not bool:
            raise _non_canonical_json()
        return value
    if tag == "uint":
        if type(value) is not int or value < 0 or value >> (8 * kind[1]):
            raise _non_canonical_json()
        return value
    if tag == "record":
        return _decode_json_record_v1(kind, value)
    raise _undecoded_detail()


def _decode_json_record_v1(kind: tuple, value: Any) -> Dict[str, Any]:
    """Keys in declaration order, every required field present, no other key.

    Absent options are omitted, never ``null``; the caller's canonical
    re-rendering also rejects repeated keys, which ``json.loads`` merges.
    """

    if type(value) is not dict:
        raise _non_canonical_json()
    fields = kind[1]
    decoded: Dict[str, Any] = {}
    index = 0
    for key in value:
        while index < len(fields) and fields[index][0] != key:
            if not fields[index][2]:
                raise _non_canonical_json()
            index += 1
        if index == len(fields):
            raise _non_canonical_json()
        _name, field_kind, _optional_field = fields[index]
        decoded[key] = _decode_json_value_v1(field_kind, value[key])
        index += 1
    if any(not optional for _name, _kind, optional in fields[index:]):
        raise _non_canonical_json()
    return decoded


def _finish_envelope_v1(envelope: Dict[str, Any]) -> ToriiErrorEnvelopeV1:
    code = envelope["code"]
    # Torii never emits an empty code; such a body is not a typed refusal.
    if not code:
        raise ValueError("error envelope code must be non-empty")
    return code, envelope["message"], envelope.get("details")


def decode_norito_error_envelope_v1(body: bytes) -> ToriiErrorEnvelopeV1:
    """Decode one canonical, unpadded Norito ``ErrorEnvelope`` frame."""

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
    return _finish_envelope_v1(_decode_norito_record_v1(_ERROR_ENVELOPE_V1, payload))


def decode_json_error_envelope_v1(body: bytes) -> ToriiErrorEnvelopeV1:
    """Decode Torii's Norito JSON rendering of an ``ErrorEnvelope``.

    The body must be exactly ``{"code":…,"message":…}`` or
    ``{"code":…,"message":…,"details":{…}}`` in strict UTF-8, with Norito's
    key order and string escaping and no insignificant whitespace.
    """

    envelope = _decode_json_record_v1(
        _ERROR_ENVELOPE_V1, json.loads(body.decode("utf-8", errors="strict"))
    )
    # Without ASCII escaping, json.dumps escapes strings exactly as Norito JSON
    # does: quote, backslash, the short control escapes and lowercase \u00xx.
    canonical = json.dumps(envelope, ensure_ascii=False, separators=(",", ":")).encode("utf-8")
    if not hmac.compare_digest(canonical, body):
        raise _non_canonical_json()
    return _finish_envelope_v1(envelope)


def decode_typed_error_envelope_v1(
    content_type: Optional[str], body: bytes
) -> Optional[ToriiErrorEnvelopeV1]:
    """Classify a non-success body as Torii's canonical error envelope.

    The declared media type selects the representation as the Rust client does:
    ``application/x-norito`` is Norito; an absent type, ``application/json`` or an
    ``application/*+json`` type is JSON. Any other media type, and any body that
    is not exactly one canonical envelope these decoders accept, returns ``None``.
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
    "ToriiErrorEnvelopeV1",
    "decode_json_error_envelope_v1",
    "decode_norito_error_envelope_v1",
    "decode_typed_error_envelope_v1",
]
