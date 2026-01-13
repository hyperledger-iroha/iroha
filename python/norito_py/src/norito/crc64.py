# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""CRC64-ECMA implementation used by the Norito codec."""

from __future__ import annotations

from functools import lru_cache

POLY = 0x42F0E1EBA9EA3693
_MASK = 0xFFFFFFFFFFFFFFFF


def _build_table() -> list[int]:
    table: list[int] = []
    for byte in range(256):
        crc = byte << 56
        for _ in range(8):
            if (crc & (1 << 63)) != 0:
                crc = ((crc << 1) ^ POLY) & _MASK
            else:
                crc = (crc << 1) & _MASK
        table.append(crc)
    return table


_TABLE = _build_table()


def crc64(data: bytes, initial: int = 0) -> int:
    """Compute the CRC64-ECMA checksum for *data*.

    Args:
        data: Input byte sequence.
        initial: Initial CRC value. Defaults to 0 per Norito spec.

    Returns:
        CRC64 checksum as unsigned 64-bit integer.
    """

    crc = initial & _MASK
    for byte in data:
        table_index = ((crc >> 56) ^ byte) & 0xFF
        crc = ((_TABLE[table_index] ^ (crc << 8)) & _MASK)
    return crc


@lru_cache(maxsize=None)
def crc64_concat(parts: tuple[bytes, ...]) -> int:
    """Convenience helper to hash concatenated byte slices deterministically."""

    crc = 0
    for part in parts:
        crc = crc64(part, crc)
    return crc


__all__ = ["crc64", "crc64_concat", "POLY"]
