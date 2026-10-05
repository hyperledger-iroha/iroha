"""Exact canonical public-key multihash literals shared by Torii client decoders.

Sumeragi status keys and governance signer lists use the same rule: hex of
``varint code || varint length || payload`` with a supported algorithm code, the
exact payload shape of that algorithm, a valid prime-order Ed25519 key for
Ed25519, and the canonical spelling (lowercase header, uppercase payload).
"""

from __future__ import annotations

import re
from typing import Any

from .vpn_validation import _is_canonical_prime_order_ed25519_public_key

__all__ = ["decode_canonical_public_key_multihash"]

# Multihash code -> (Norito algorithm ordinal, exact payload bytes; SM2: SEC1 point bytes).
_PUBLIC_KEY_SHAPES = {
    0xED: (0, 32),
    0xE7: (1, 33),
    0xEA: (2, 48),
    0xEB: (3, 96),
    0xEE: (4, 1952),
    0x1200: (5, 64),
    0x1201: (6, 64),
    0x1202: (7, 64),
    0x1203: (8, 128),
    0x1204: (9, 128),
    0x1306: (10, 65),
}


def _exact_text(value: Any, context: str) -> str:
    if not isinstance(value, str) or value != value.strip() or any(ord(ch) < 32 or ord(ch) == 127 for ch in value):
        raise TypeError(f"{context} must be exact text without surrounding whitespace")
    if not value:
        raise TypeError(f"{context} must be non-empty")
    return value


def _multihash_varint(data: bytes, start: int, context: str) -> tuple[int, int]:
    value = 0
    shift = 0
    for offset in range(start, min(len(data), start + 10)):
        byte = data[offset]
        chunk = byte & 0x7F
        if shift == 63 and chunk > 1:
            break
        value |= chunk << shift
        if byte & 0x80 == 0:
            length = offset + 1 - start
            if length > 1 and chunk == 0:
                break
            return value, offset + 1
        shift += 7
    raise TypeError(f"{context} has a malformed or noncanonical multihash varint")


def decode_canonical_public_key_multihash(value: Any, context: str) -> tuple[str, tuple[int, bytes]]:
    """Return the literal and its ``(algorithm ordinal, payload)`` ordering key.

    Raises ``TypeError`` naming ``context`` for any other value.
    """
    literal = _exact_text(value, context)
    if len(literal) > 1_048_576 or len(literal) % 2 or re.fullmatch(r"[0-9a-fA-F]+", literal) is None:
        raise TypeError(f"{context} must be a bare canonical public-key multihash")
    data = bytes.fromhex(literal)
    code, code_end = _multihash_varint(data, 0, context)
    length, payload_start = _multihash_varint(data, code_end, context)
    shape = _PUBLIC_KEY_SHAPES.get(code)
    payload = data[payload_start:]
    if shape is None or length == 0 or length != len(payload):
        raise TypeError(f"{context} has an unsupported public-key algorithm or length")
    if literal != data[:payload_start].hex() + payload.hex().upper():
        raise TypeError(f"{context} must be an exact canonical public-key multihash")
    ordinal, expected_length = shape
    if code == 0x1306:
        if len(payload) < 2 + expected_length:
            raise TypeError(f"{context} has an invalid SM2 public-key payload")
        distid_length = int.from_bytes(payload[:2], "big")
        sec1_start = 2 + distid_length
        if (
            distid_length > 0xFFFF // 8
            or len(payload) != sec1_start + expected_length
            or payload[sec1_start] != 0x04
        ):
            raise TypeError(f"{context} has an invalid SM2 public-key payload")
        try:
            payload[2:sec1_start].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise TypeError(f"{context} has an invalid UTF-8 SM2 distinguished ID") from exc
    elif len(payload) != expected_length:
        raise TypeError(f"{context} has an invalid public-key payload length")
    if code == 0xE7 and payload[0] not in (0x02, 0x03):
        raise TypeError(f"{context} has an invalid secp256k1 public-key envelope")
    if code == 0xEE and not any(payload):
        raise TypeError(f"{context} has an all-zero ML-DSA public key")
    if code == 0xED and not _is_canonical_prime_order_ed25519_public_key(payload):
        raise TypeError(f"{context} must contain a valid prime-order Ed25519 public key")
    return literal, (ordinal, payload)
