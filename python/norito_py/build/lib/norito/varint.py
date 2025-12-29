# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""7-bit little-endian varint helpers."""

from __future__ import annotations

from typing import Iterator, Tuple


class VarintError(ValueError):
    """Raised when varint decoding exceeds limits."""


def encode_varint(value: int) -> bytes:
    """Encode an unsigned integer using 7-bit little-endian varint encoding."""

    if value < 0:
        raise VarintError("varint cannot encode negative values")
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            break
    return bytes(out)


def decode_varint(data: bytes, offset: int = 0) -> Tuple[int, int]:
    """Decode a varint from *data* starting at *offset*.

    Returns a tuple ``(value, next_offset)``.
    """

    shift = 0
    result = 0
    idx = offset
    while True:
        if idx >= len(data):
            raise VarintError("unexpected end of data while decoding varint")
        byte = data[idx]
        idx += 1
        result |= (byte & 0x7F) << shift
        if (byte & 0x80) == 0:
            break
        shift += 7
        if shift >= 64:
            raise VarintError("varint exceeds 64 bits")
    return result, idx


def iter_varints(data: bytes) -> Iterator[int]:
    """Yield successive varints from *data*."""

    offset = 0
    while offset < len(data):
        value, offset = decode_varint(data, offset)
        yield value


__all__ = ["encode_varint", "decode_varint", "iter_varints", "VarintError"]
