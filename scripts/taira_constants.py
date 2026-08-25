#!/usr/bin/env python3
"""Canonical first-release public Taira identity and validator projection."""

from __future__ import annotations


NETWORK_NAME = "taira"
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
NETWORK_ID = "82531ce8eae8bff6beeca4698bfd13a3bc8bec5f0ee0d23d428c97fc17ab0f3b"
CHAIN_DISCRIMINANT = 369
PEER_COUNT = 4
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))


def network_id_from_genesis_hash(genesis_hash: str) -> str:
    """Validate and return the canonical raw NetworkId for one reset genesis hash."""

    if (
        not isinstance(genesis_hash, str)
        or len(genesis_hash) != 64
        or genesis_hash != genesis_hash.lower()
        or any(character not in "0123456789abcdef" for character in genesis_hash)
        or int(genesis_hash[-2:], 16) & 1 == 0
    ):
        raise ValueError(
            "genesis hash must be one lowercase 32-byte digest with its marker bit set"
        )
    return genesis_hash


def norito_hash_literal_from_genesis_hash(genesis_hash: str) -> str:
    """Render one raw genesis hash for a typed Norito JSON/TOML hash field."""

    body = network_id_from_genesis_hash(genesis_hash).upper()
    crc = 0xFFFF
    for byte in f"hash:{body}".encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return f"hash:{body}#{crc:04X}"
