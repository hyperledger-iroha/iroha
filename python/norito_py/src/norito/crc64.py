# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""CRC64-XZ implementation used by the Norito codec."""

from __future__ import annotations

from functools import lru_cache

POLY = 0xC96C5795D7870F42
_MASK = 0xFFFFFFFFFFFFFFFF
_INIT = _MASK
_XOR_OUT = _MASK


def _build_table() -> list[int]:
    table: list[int] = []
    for byte in range(256):
        crc = byte
        for _ in range(8):
            if crc & 1:
                crc = (crc >> 1) ^ POLY
            else:
                crc >>= 1
        table.append(crc)
    return table


_TABLE = _build_table()


def _update_crc64(data: bytes, initial: int) -> int:
    crc = initial & _MASK
    for byte in data:
        table_index = (crc ^ byte) & 0xFF
        crc = (_TABLE[table_index] ^ (crc >> 8)) & _MASK
    return crc


def crc64(data: bytes, initial: int = _INIT) -> int:
    """Compute the CRC64-XZ checksum for *data*.

    Args:
        data: Input byte sequence.
        initial: Initial CRC state before final XOR-out. Defaults to all ones.

    Returns:
        CRC64 checksum as unsigned 64-bit integer.
    """

    crc = _update_crc64(data, initial)
    return crc ^ _XOR_OUT


@lru_cache(maxsize=None)
def crc64_concat(parts: tuple[bytes, ...]) -> int:
    """Convenience helper to hash concatenated byte slices deterministically."""

    crc = _INIT
    for part in parts:
        crc = _update_crc64(part, crc)
    return crc ^ _XOR_OUT


__all__ = ["crc64", "crc64_concat", "POLY"]
