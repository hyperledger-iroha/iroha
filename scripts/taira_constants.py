#!/usr/bin/env python3
"""Canonical first-release public Taira identity and validator projection."""

from __future__ import annotations


NETWORK_NAME = "taira"
CHAIN_ID = "fc56984b-2be7-431d-840e-21514d1883f0"
NETWORK_ID = "hash:82531CE8EAE8BFF6BEECA4698BFD13A3BC8BEC5F0EE0D23D428C97FC17AB0F3B#3E94"
CHAIN_DISCRIMINANT = 369
PEER_COUNT = 4
SLUGS = tuple(f"taira-validator-{index}" for index in range(1, PEER_COUNT + 1))


def canonical_network_id(network_id: str) -> str:
    """Validate and return one canonical checked NetworkId literal."""

    prefix = "hash:"
    if not isinstance(network_id, str) or not network_id.startswith(prefix):
        raise ValueError("network id must use the canonical checked hash literal")
    try:
        body, checksum = network_id[len(prefix) :].split("#", maxsplit=1)
    except ValueError as error:
        raise ValueError("network id must use the canonical checked hash literal") from error
    if (
        len(body) != 64
        or body != body.upper()
        or any(character not in "0123456789ABCDEF" for character in body)
        or len(checksum) != 4
        or checksum != checksum.upper()
        or any(character not in "0123456789ABCDEF" for character in checksum)
    ):
        raise ValueError("network id must use the canonical checked hash literal")
    canonical = network_id_from_genesis_hash(body.lower())
    if network_id != canonical:
        raise ValueError("network id checksum does not match its genesis hash")
    return canonical


def network_id_from_genesis_hash(genesis_hash: str) -> str:
    """Return the canonical CRC-bound NetworkId for one reset genesis hash."""

    if (
        not isinstance(genesis_hash, str)
        or len(genesis_hash) != 64
        or genesis_hash != genesis_hash.lower()
        or genesis_hash == "0" * 64
        or any(character not in "0123456789abcdef" for character in genesis_hash)
        or genesis_hash[-1] not in "13579bdf"
    ):
        raise ValueError("genesis hash must be one canonical marked lowercase 32-byte digest")
    body = genesis_hash.upper()
    crc = 0xFFFF
    for byte in b"hash:" + body.encode("ascii"):
        crc ^= byte << 8
        for _ in range(8):
            if crc & 0x8000:
                crc = ((crc << 1) ^ 0x1021) & 0xFFFF
            else:
                crc = (crc << 1) & 0xFFFF
    return f"hash:{body}#{crc:04X}"
