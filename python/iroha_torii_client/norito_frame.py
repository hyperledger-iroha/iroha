"""Strict, payload-opaque validation for canonical Norito v1 frames."""

from __future__ import annotations

import hashlib
from typing import Optional

_HEADER_BYTES = 40
_MAX_HEADER_PADDING_BYTES = 64
_COMPACT_LEN_FLAG = 0x02
_PACKED_STRUCT_FLAG = 0x04
_FIELD_BITSET_FLAG = 0x20
_SUPPORTED_FLAGS_MASK = (
    0x01 | _COMPACT_LEN_FLAG | _PACKED_STRUCT_FLAG | _FIELD_BITSET_FLAG
)
_CRC64_MASK = 0xFFFF_FFFF_FFFF_FFFF
_CRC64_REFLECTED_POLY = 0xC96C_5795_D787_0F42


def _build_crc64_table() -> tuple[int, ...]:
    table = []
    for value in range(256):
        crc = value
        for _ in range(8):
            crc = (crc >> 1) ^ (_CRC64_REFLECTED_POLY if crc & 1 else 0)
        table.append(crc & _CRC64_MASK)
    return tuple(table)


_CRC64_TABLE = _build_crc64_table()


def _crc64_xz(payload: bytes) -> int:
    crc = _CRC64_MASK
    for byte in payload:
        crc = _CRC64_TABLE[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return (crc ^ _CRC64_MASK) & _CRC64_MASK


def schema_hash_for_type_name(type_name: str) -> bytes:
    """Return Norito's domain-separated 16-byte type-name schema hash."""

    if not isinstance(type_name, str) or not type_name:
        raise TypeError("Norito type name must be a nonempty string")
    return hashlib.sha256(
        b"norito:v1:type-name\0" + type_name.encode("utf-8")
    ).digest()[:16]


def encode_norito_frame(
    payload: bytes,
    *,
    type_name: str,
    flags: int = 0,
    payload_alignment: int = 1,
) -> bytes:
    """Frame one immutable payload with an exact type-name schema binding."""

    if type(payload) is not bytes or not payload:
        raise TypeError("Norito payload must be non-empty immutable bytes")
    if type(flags) is not int or flags < 0 or flags > 0xFF:
        raise TypeError("Norito flags must be one unsigned byte")
    if flags & ~_SUPPORTED_FLAGS_MASK:
        raise ValueError(f"unsupported Norito header flags 0x{flags:02x}")
    required_bitset_flags = _PACKED_STRUCT_FLAG | _COMPACT_LEN_FLAG
    if flags & _FIELD_BITSET_FLAG and flags & required_bitset_flags != required_bitset_flags:
        raise ValueError("invalid Norito header flag combination")
    if (
        type(payload_alignment) is not int
        or payload_alignment < 1
        or payload_alignment > _MAX_HEADER_PADDING_BYTES
        or payload_alignment & (payload_alignment - 1)
    ):
        raise TypeError("Norito payload alignment must be a power of two from 1 through 64")
    padding_length = (-_HEADER_BYTES) % payload_alignment
    return b"".join(
        (
            b"NRT0",
            b"\x00\x00",
            schema_hash_for_type_name(type_name),
            b"\x00",
            len(payload).to_bytes(8, "little"),
            _crc64_xz(payload).to_bytes(8, "little"),
            flags.to_bytes(1, "little"),
            bytes(padding_length),
            payload,
        )
    )


def _validate_norito_frame(
    body: bytes,
    *,
    context: str,
    expected_type_name: Optional[str],
    expected_padding_length: Optional[int] = None,
    expected_flags: Optional[int] = None,
    require_nonempty_payload: bool = True,
) -> bytes:
    """Validate one uncompressed Norito frame without decoding its payload."""

    if not isinstance(body, bytes) or len(body) < _HEADER_BYTES:
        raise ValueError(f"{context} is shorter than the {_HEADER_BYTES}-byte Norito header")
    if body[:4] != b"NRT0":
        raise ValueError(f"{context} is not an NRT0 frame")
    major, minor = body[4], body[5]
    if major != 0 or minor != 0:
        raise ValueError(f"{context} uses unsupported NRT0 version {major}.{minor}")
    if body[6:22] == bytes(16):
        raise ValueError(f"{context} must advertise a non-zero Norito schema hash")
    if (
        expected_type_name is not None
        and body[6:22] != schema_hash_for_type_name(expected_type_name)
    ):
        raise ValueError(f"{context} schema hash did not match the expected type")
    if body[22] != 0:
        raise ValueError(f"{context} must use uncompressed Norito payload encoding")

    payload_length = int.from_bytes(body[23:31], "little")
    if require_nonempty_payload and payload_length == 0:
        raise ValueError(f"{context} must contain a non-empty Norito payload")
    expected_checksum = int.from_bytes(body[31:39], "little")
    flags = body[39]
    if flags & ~_SUPPORTED_FLAGS_MASK:
        raise ValueError(f"{context} uses unsupported Norito header flags 0x{flags:02x}")
    required_bitset_flags = _PACKED_STRUCT_FLAG | _COMPACT_LEN_FLAG
    if flags & _FIELD_BITSET_FLAG and flags & required_bitset_flags != required_bitset_flags:
        raise ValueError(f"{context} uses an invalid Norito header flag combination")
    if expected_flags is not None and flags != expected_flags:
        raise ValueError(
            f"{context} uses Norito layout flags 0x{flags:02x}; "
            f"the exact type requires 0x{expected_flags:02x}"
        )

    padding_length = len(body) - _HEADER_BYTES - payload_length
    if padding_length < 0:
        raise ValueError(f"{context} payload length exceeds the available frame bytes")
    if padding_length > _MAX_HEADER_PADDING_BYTES:
        raise ValueError(
            f"{context} exceeds the {_MAX_HEADER_PADDING_BYTES}-byte Norito header-padding bound"
        )
    if expected_padding_length is not None and padding_length != expected_padding_length:
        raise ValueError(
            f"{context} uses {padding_length} bytes of alignment padding; "
            f"the exact type requires {expected_padding_length}"
        )
    payload_start = _HEADER_BYTES + padding_length
    if any(body[_HEADER_BYTES:payload_start]):
        raise ValueError(f"{context} contains non-zero alignment padding or trailing bytes")
    payload_end = payload_start + payload_length
    payload = body[payload_start:payload_end]
    if len(payload) != payload_length or payload_end != len(body):
        raise ValueError(f"{context} contains trailing bytes outside the declared payload")
    if _crc64_xz(payload) != expected_checksum:
        raise ValueError(f"{context} CRC64 mismatch")
    return payload


def validate_norito_frame(
    body: bytes,
    *,
    context: str,
    expected_type_name: str,
    expected_padding_length: Optional[int] = None,
    expected_flags: Optional[int] = None,
    require_nonempty_payload: bool = True,
) -> None:
    """Validate one exact-schema, uncompressed Norito frame."""

    _validate_norito_frame(
        body,
        context=context,
        expected_type_name=expected_type_name,
        expected_padding_length=expected_padding_length,
        expected_flags=expected_flags,
        require_nonempty_payload=require_nonempty_payload,
    )


def decode_norito_frame_payload(
    body: bytes,
    *,
    context: str,
    expected_type_name: str,
    expected_padding_length: Optional[int] = None,
    expected_flags: Optional[int] = None,
    require_nonempty_payload: bool = True,
) -> bytes:
    """Validate one exact-schema frame and return its immutable payload."""

    return _validate_norito_frame(
        body,
        context=context,
        expected_type_name=expected_type_name,
        expected_padding_length=expected_padding_length,
        expected_flags=expected_flags,
        require_nonempty_payload=require_nonempty_payload,
    )


def validate_opaque_norito_frame(
    body: bytes,
    *,
    context: str,
    expected_padding_length: Optional[int] = None,
    expected_flags: Optional[int] = None,
    require_nonempty_payload: bool = True,
) -> None:
    """Validate framing only for an explicitly opaque, native-decoded envelope."""

    _validate_norito_frame(
        body,
        context=context,
        expected_type_name=None,
        expected_padding_length=expected_padding_length,
        expected_flags=expected_flags,
        require_nonempty_payload=require_nonempty_payload,
    )
