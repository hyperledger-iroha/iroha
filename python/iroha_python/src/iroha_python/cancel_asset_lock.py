"""Strict bare ``CancelAssetLock`` V1 JSON fields and Norito archive codec."""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass
from typing import Any, Final

from .numeric_v1 import MAX_MANTISSA_BYTES, KotodamaQuantity, NumericV1Codec

CANCEL_ASSET_LOCK_WIRE_ID_V1: Final[str] = "iroha_data_model::isi::escrow::CancelAssetLock"
"""Schema type name for the first-release bare cancellation archive."""

_FRAME_HEADER_BYTES: Final[int] = 40
_COMPACT_LENGTH_FLAG: Final[int] = 0x02
# A transparent 32-byte EscrowId plus one positive signed-512-bit Quantity
# yields an unpadded canonical archive in this exact range. Enforce it before
# CRC work so an oversized attacker-controlled frame cannot make this fixed
# schema perform an unbounded payload scan.
_MIN_CANONICAL_ARCHIVE_BYTES: Final[int] = 85
_MAX_CANONICAL_ARCHIVE_BYTES: Final[int] = 148
_U64_MASK: Final[int] = (1 << 64) - 1
_CRC64_POLY: Final[int] = 0xC96C_5795_D787_0F42
_SCHEMA_HASH: Final[bytes] = hashlib.sha256(
    b"norito:v1:type-name\0" + CANCEL_ASSET_LOCK_WIRE_ID_V1.encode("ascii")
).digest()[:16]
_CANONICAL_HASH_LITERAL_RE: Final[re.Pattern[str]] = re.compile(
    r"hash:([0-9A-F]{64})#([0-9A-F]{4})\Z",
    re.ASCII,
)


def _crc64(payload: bytes) -> int:
    crc = _U64_MASK
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = ((crc >> 1) ^ _CRC64_POLY) if crc & 1 else crc >> 1
    return (crc ^ _U64_MASK) & _U64_MASK


def _crc16(payload: bytes) -> int:
    crc = 0xFFFF
    for byte in payload:
        crc ^= byte << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) & 0xFFFF if crc & 0x8000 else (crc << 1) & 0xFFFF
    return crc


def _compact_length(value: int) -> bytes:
    if type(value) is not int or value < 0:
        raise TypeError("Norito compact length must be a non-negative integer")
    encoded = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        encoded.append(byte | (0x80 if remaining else 0))
        if not remaining:
            return bytes(encoded)


def _field(payload: bytes) -> bytes:
    return _compact_length(len(payload)) + payload


def _twos_complement_little_endian(value: int) -> bytes:
    if value == 0:
        return b""
    if value > 0:
        encoded = bytearray()
        remaining = value
        while remaining:
            encoded.append(remaining & 0xFF)
            remaining >>= 8
        if encoded[-1] & 0x80:
            encoded.append(0)
        return bytes(encoded)

    byte_length = 1
    while value < -(1 << (byte_length * 8 - 1)):
        byte_length += 1
    encoded_value = (1 << (byte_length * 8)) + value
    encoded = bytearray((encoded_value >> (index * 8)) & 0xFF for index in range(byte_length))
    while len(encoded) > 1 and encoded[-1] == 0xFF and encoded[-2] & 0x80:
        encoded.pop()
    return bytes(encoded)


class _CompactReader:
    def __init__(self, payload: bytes, context: str) -> None:
        self._payload = payload
        self._context = context
        self._offset = 0

    def _read(self, length: int, field: str) -> bytes:
        if length < 0 or self._offset + length > len(self._payload):
            raise ValueError(f"{self._context}.{field} overruns the Norito payload")
        result = self._payload[self._offset : self._offset + length]
        self._offset += length
        return result

    def _length(self, field: str) -> int:
        start = self._offset
        value = 0
        shift = 0
        while True:
            byte = self._read(1, f"{field}.length")[0]
            part = byte & 0x7F
            if shift == 63 and part > 1:
                raise ValueError(f"{self._context}.{field} length exceeds u64")
            value |= part << shift
            if byte & 0x80 == 0:
                break
            shift += 7
            if shift > 63:
                raise ValueError(f"{self._context}.{field} length exceeds u64")
        if self._payload[start : self._offset] != _compact_length(value):
            raise ValueError(f"{self._context}.{field} uses a noncanonical length")
        return value

    def field(self, name: str) -> bytes:
        return self._read(self._length(name), name)

    def assert_eof(self) -> None:
        if self._offset != len(self._payload):
            raise ValueError(f"{self._context} contains trailing bytes")


def _utf8_text(value: Any, context: str) -> str:
    if type(value) is not str:
        raise TypeError(f"{context} must be a string")
    try:
        value.encode("utf-8", errors="strict")
    except UnicodeEncodeError as error:
        raise ValueError(f"{context} must be valid Unicode text") from error
    return value


def _escrow_hash_bytes(value: Any) -> bytes:
    literal = _utf8_text(value, "escrow_id")
    matched = _CANONICAL_HASH_LITERAL_RE.fullmatch(literal)
    if matched is None:
        raise ValueError("escrow_id must be one canonical uppercase checksummed hash literal")
    body, checksum = matched.groups()
    expected_checksum = f"{_crc16(f'hash:{body}'.encode('ascii')):04X}"
    if checksum != expected_checksum:
        raise ValueError(f"escrow_id has an invalid checksum; expected {expected_checksum}")
    decoded = bytes.fromhex(body)
    if decoded[-1] & 1 == 0:
        raise ValueError("escrow_id must use a native hash with its marker bit set")
    return decoded


def _hash_literal(value: bytes) -> str:
    body = value.hex().upper()
    checksum = _crc16(f"hash:{body}".encode("ascii"))
    return f"hash:{body}#{checksum:04X}"


def _positive_quantity(value: Any) -> KotodamaQuantity:
    literal = _utf8_text(value, "expected_remaining_amount")
    quantity = NumericV1Codec.decode_quantity_json(literal)
    if quantity.mantissa <= 0:
        raise ValueError("expected_remaining_amount must be greater than zero")
    return quantity


def _encode_quantity(quantity: KotodamaQuantity) -> bytes:
    mantissa = _twos_complement_little_endian(quantity.mantissa)
    mantissa_payload = len(mantissa).to_bytes(4, "little") + mantissa
    return _field(mantissa_payload) + _field(quantity.scale.to_bytes(4, "little"))


def _decode_quantity(payload: bytes) -> KotodamaQuantity:
    reader = _CompactReader(payload, "CancelAssetLock.expected_remaining_amount")
    mantissa_payload = reader.field("mantissa")
    scale_payload = reader.field("scale")
    reader.assert_eof()
    if len(mantissa_payload) < 4:
        raise ValueError("quantity mantissa is truncated")
    byte_count = int.from_bytes(mantissa_payload[:4], "little")
    mantissa_bytes = mantissa_payload[4:]
    if byte_count > MAX_MANTISSA_BYTES or byte_count != len(mantissa_bytes):
        raise ValueError("quantity mantissa length is invalid")
    if len(scale_payload) != 4:
        raise ValueError("quantity scale must contain exactly four bytes")
    scale = int.from_bytes(scale_payload, "little")
    if scale > 28:
        raise ValueError("quantity scale exceeds 28")
    mantissa = int.from_bytes(mantissa_bytes, "little", signed=True) if mantissa_bytes else 0
    if _twos_complement_little_endian(mantissa) != mantissa_bytes:
        raise ValueError("quantity mantissa is not minimally encoded")
    if mantissa <= 0:
        raise ValueError("expected_remaining_amount must be greater than zero")
    quantity = KotodamaQuantity(mantissa, scale)
    if quantity.mantissa != mantissa or quantity.scale != scale:
        raise ValueError("expected_remaining_amount is not canonically encoded")
    if _encode_quantity(quantity) != payload:
        raise ValueError("expected_remaining_amount is not byte-canonical")
    return quantity


@dataclass(frozen=True)
class CancelAssetLockV1:
    """Exact two-field value carried by a bare cancellation V1 archive."""

    escrow_id: str
    expected_remaining_amount: str

    def __post_init__(self) -> None:
        _escrow_hash_bytes(self.escrow_id)
        _positive_quantity(self.expected_remaining_amount)

    def to_mapping(self) -> dict[str, str]:
        """Return the exact bare Norito JSON field mapping."""

        return {
            "escrow_id": self.escrow_id,
            "expected_remaining_amount": self.expected_remaining_amount,
        }

    def encode(self) -> bytes:
        """Encode this value as the schema-bound bare V1 archive."""

        return encode_cancel_asset_lock_v1(
            self.escrow_id,
            self.expected_remaining_amount,
        )


def encode_cancel_asset_lock_v1(
    escrow_id: str,
    expected_remaining_amount: str,
) -> bytes:
    """Encode exact canonical fields as a schema-bound bare V1 archive."""

    hash_bytes = _escrow_hash_bytes(escrow_id)
    quantity = _positive_quantity(expected_remaining_amount)
    payload = _field(hash_bytes) + _field(_encode_quantity(quantity))
    header = (
        b"NRT0\x00\x00"
        + _SCHEMA_HASH
        + b"\x00"
        + len(payload).to_bytes(8, "little")
        + _crc64(payload).to_bytes(8, "little")
        + bytes([_COMPACT_LENGTH_FLAG])
    )
    return header + payload


def decode_cancel_asset_lock_v1(archive: bytes) -> CancelAssetLockV1:
    """Decode one exact schema-bound bare V1 archive without container aliases."""

    if type(archive) is not bytes:
        raise TypeError("archive must be exact bytes")
    encoded = archive
    if len(encoded) < _FRAME_HEADER_BYTES:
        raise ValueError("CancelAssetLockV1 archive is shorter than its Norito header")
    if not _MIN_CANONICAL_ARCHIVE_BYTES <= len(encoded) <= _MAX_CANONICAL_ARCHIVE_BYTES:
        raise ValueError(
            "CancelAssetLockV1 archive must contain between "
            f"{_MIN_CANONICAL_ARCHIVE_BYTES} and "
            f"{_MAX_CANONICAL_ARCHIVE_BYTES} canonical bytes"
        )
    if encoded[:6] != b"NRT0\x00\x00":
        raise ValueError("CancelAssetLockV1 archive has invalid Norito magic or version")
    if encoded[6:22] != _SCHEMA_HASH:
        raise ValueError("CancelAssetLockV1 archive has the wrong schema")
    if encoded[22] != 0:
        raise ValueError("CancelAssetLockV1 archive must be uncompressed")
    if encoded[39] != _COMPACT_LENGTH_FLAG:
        raise ValueError("CancelAssetLockV1 archive must use exactly the compact-length flag")
    declared_length = int.from_bytes(encoded[23:31], "little")
    if declared_length != len(encoded) - _FRAME_HEADER_BYTES:
        raise ValueError("CancelAssetLockV1 archive must be unpadded and contain no trailing bytes")
    payload = encoded[_FRAME_HEADER_BYTES:]
    if int.from_bytes(encoded[31:39], "little") != _crc64(payload):
        raise ValueError("CancelAssetLockV1 archive has an invalid CRC64")

    reader = _CompactReader(payload, "CancelAssetLockV1")
    escrow_bytes = reader.field("escrow_id")
    quantity_bytes = reader.field("expected_remaining_amount")
    reader.assert_eof()
    if len(escrow_bytes) != 32 or escrow_bytes[-1] & 1 == 0:
        raise ValueError("escrow_id must be a transparent 32-byte marked hash")
    quantity = _decode_quantity(quantity_bytes)
    decoded = CancelAssetLockV1(
        escrow_id=_hash_literal(escrow_bytes),
        expected_remaining_amount=str(quantity),
    )
    if decoded.encode() != encoded:
        raise ValueError("CancelAssetLockV1 archive is not byte-canonical")
    return decoded


__all__ = [
    "CANCEL_ASSET_LOCK_WIRE_ID_V1",
    "CancelAssetLockV1",
    "decode_cancel_asset_lock_v1",
    "encode_cancel_asset_lock_v1",
]
