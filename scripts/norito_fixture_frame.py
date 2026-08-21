"""Strict canonical Norito framing for cross-SDK transaction fixtures."""

from __future__ import annotations

import hashlib
import struct

HEADER_LENGTH = 40
CANONICAL_FLAGS = 0x02
TRANSACTION_PAYLOAD_TYPE = (
    "iroha_data_model::transaction::signed::model::TransactionPayload"
)
SIGNED_TRANSACTION_TYPE = (
    "iroha_data_model::transaction::signed::model::SignedTransaction"
)


def _schema_hash(type_name: str) -> bytes:
    domain = b"norito:v1:type-name\x00"
    return hashlib.sha256(domain + type_name.encode("utf-8")).digest()[:16]


TRANSACTION_PAYLOAD_SCHEMA = _schema_hash(TRANSACTION_PAYLOAD_TYPE)
SIGNED_TRANSACTION_SCHEMA = _schema_hash(SIGNED_TRANSACTION_TYPE)
CANONICAL_PADDING_BY_SCHEMA = {
    TRANSACTION_PAYLOAD_SCHEMA: 0,
    SIGNED_TRANSACTION_SCHEMA: 0,
}


def _crc64_xz(data: bytes) -> int:
    crc = 0xFFFF_FFFF_FFFF_FFFF
    polynomial = 0xC96C_5795_D787_0F42
    for value in data:
        crc ^= value
        for _ in range(8):
            crc = (crc >> 1) ^ polynomial if crc & 1 else crc >> 1
    return crc ^ 0xFFFF_FFFF_FFFF_FFFF


def decode_canonical_norito_frame(
    frame: bytes, context: str, *, expected_schema: bytes
) -> bytes:
    """Return one uncompressed V1 payload, rejecting bare or ambiguous bytes."""

    if len(frame) < HEADER_LENGTH:
        raise ValueError(f"{context} is shorter than the mandatory Norito header")
    if frame[:4] != b"NRT0" or frame[4:6] != b"\x00\x00":
        raise ValueError(f"{context} is not a canonical NRT0 V1 frame")
    if len(expected_schema) != 16 or frame[6:22] != expected_schema:
        raise ValueError(f"{context} does not use its required canonical schema hash")
    if frame[22] != 0:
        raise ValueError(f"{context} uses forbidden compression")
    expected_padding = CANONICAL_PADDING_BY_SCHEMA.get(expected_schema)
    if expected_padding is None:
        raise ValueError(f"{context} does not name a supported fixture schema")
    if frame[39] != CANONICAL_FLAGS:
        raise ValueError(f"{context} does not use the canonical fixture flags")
    payload_length = struct.unpack_from("<Q", frame, 23)[0]
    if payload_length == 0:
        raise ValueError(f"{context} has an empty payload")
    checksum = struct.unpack_from("<Q", frame, 31)[0]
    padding_length = len(frame) - HEADER_LENGTH - payload_length
    if padding_length != expected_padding:
        raise ValueError(f"{context} does not use its exact canonical padding")
    payload_start = HEADER_LENGTH + padding_length
    if any(frame[HEADER_LENGTH:payload_start]):
        raise ValueError(f"{context} has non-zero alignment padding")
    payload = frame[payload_start:]
    if len(payload) != payload_length or _crc64_xz(payload) != checksum:
        raise ValueError(f"{context} fails its canonical length or CRC64 check")
    return payload


def iroha_hash_hex(data: bytes) -> str:
    """Return the canonical Iroha BLAKE2b-256 hash spelling."""

    digest = bytearray(hashlib.blake2b(data, digest_size=32).digest())
    digest[-1] |= 1
    return digest.hex()


def _compact_length(value: int) -> bytes:
    if value < 0:
        raise ValueError("compact length must be non-negative")
    encoded = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            byte |= 0x80
        encoded.append(byte)
        if not value:
            return bytes(encoded)


def _decode_compact_length(data: bytes, offset: int) -> tuple[int, int]:
    start = offset
    value = 0
    shift = 0
    while offset < len(data) and shift <= 63:
        byte = data[offset]
        offset += 1
        value |= (byte & 0x7F) << shift
        if byte & 0x80 == 0:
            if data[start:offset] != _compact_length(value):
                raise ValueError("non-canonical compact field length")
            return value, offset
        shift += 7
    raise ValueError("truncated or overflowing compact field length")


def _read_field(data: bytes, offset: int, context: str) -> tuple[bytes, int]:
    length, payload_offset = _decode_compact_length(data, offset)
    end = payload_offset + length
    if end > len(data):
        raise ValueError(f"truncated {context}")
    return data[payload_offset:end], end


def signed_transaction_payload(data: bytes) -> bytes:
    """Extract the payload from the exact first-release signed envelope."""

    _, offset = _read_field(data, 0, "SignedTransaction.signature")
    payload, offset = _read_field(data, offset, "SignedTransaction.payload")
    _, offset = _read_field(data, offset, "SignedTransaction.multisig_signatures")
    if offset != len(data):
        raise ValueError("SignedTransaction has trailing or legacy envelope fields")
    return payload


def signed_transaction_entrypoint_hash_hex(data: bytes) -> str:
    """Hash the signed transaction's payload in the compact External domain."""

    payload = signed_transaction_payload(data)
    preimage = b"\x00\x00\x00\x00" + _compact_length(len(payload)) + payload
    return iroha_hash_hex(preimage)
