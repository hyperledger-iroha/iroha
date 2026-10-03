# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""CRC-32C (Castagnoli) used to bind a reassembled payload to its beacon."""

from __future__ import annotations

__all__ = ["crc32c"]

#: Reflected CRC-32C polynomial.
_POLY = 0x82F63B78


def _make_table() -> tuple:
    table = []
    for index in range(256):
        crc = index
        for _ in range(8):
            crc = (crc >> 1) ^ _POLY if crc & 1 else crc >> 1
        table.append(crc)
    return tuple(table)


_TABLE = _make_table()


def crc32c(data: bytes) -> int:
    """Compute CRC-32C (init ``0xFFFFFFFF``, reflected, final xor ``0xFFFFFFFF``)."""
    table = _TABLE
    crc = 0xFFFFFFFF
    for byte in bytes(data):
        crc = table[(crc ^ byte) & 0xFF] ^ (crc >> 8)
    return crc ^ 0xFFFFFFFF
