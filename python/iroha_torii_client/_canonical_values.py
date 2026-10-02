"""Canonical asset identifiers and Quantity DATA shared by Torii readers."""

from __future__ import annotations

import re
from typing import Any, Sequence

from blake3 import blake3

from . import _account_id as _account_id_codec

BASE58_ALPHABET = _account_id_codec.BASE58_ALPHABET
BASE58_INDEX = {symbol: idx for idx, symbol in enumerate(BASE58_ALPHABET)}
_QUANTITY_MAX_TEXT_LENGTH = 155
_QUANTITY_MAX_MANTISSA = (1 << 511) - 1
_OFFLINE_ASSET_DEFINITION_ID_RE = re.compile(r"^[1-9A-HJ-NP-Za-km-z]{28}$")


def _canonical_quantity(value: Any, context: str) -> str:
    """Decode one canonical bounded non-negative Quantity JSON string."""

    if not isinstance(value, str):
        raise RuntimeError(f"{context} must be a quantity string")
    if len(value) > _QUANTITY_MAX_TEXT_LENGTH:
        raise RuntimeError(f"{context} quantity exceeds the text length bound")
    matched = re.fullmatch(r"(0|[1-9][0-9]*)(?:\.([0-9]{0,27}[1-9]))?", value)
    if matched is None:
        raise RuntimeError(f"{context} must be a canonical non-negative quantity")
    fraction = matched.group(2) or ""
    mantissa = int(matched.group(1) + fraction)
    if mantissa > _QUANTITY_MAX_MANTISSA:
        raise RuntimeError(f"{context} quantity exceeds the signed 512-bit domain")
    return value


def _decode_base_n(digits: Sequence[int], base: int) -> bytes:
    value = 0
    for digit in digits:
        value = value * base + digit
    if value == 0:
        decoded = b""
    else:
        pieces = bytearray()
        while value:
            pieces.append(value & 0xFF)
            value >>= 8
        decoded = bytes(reversed(pieces))
    pad = 0
    for digit in digits:
        if digit == 0:
            pad += 1
        else:
            break
    return b"\x00" * pad + decoded


def _offline_exact_string(value: Any, context: str, *, non_empty: bool = True) -> str:
    if not isinstance(value, str):
        raise RuntimeError(f"{context} must be a string")
    if non_empty and not value:
        raise RuntimeError(f"{context} must not be empty")
    if value.strip() != value:
        raise RuntimeError(f"{context} must not contain surrounding whitespace")
    if any(0xD800 <= ord(character) <= 0xDFFF for character in value):
        raise RuntimeError(f"{context} must not contain Unicode surrogate code points")
    if any(ord(character) < 0x20 or 0x7F <= ord(character) <= 0x9F for character in value):
        raise RuntimeError(f"{context} must not contain control characters")
    return value


def _offline_canonical_asset_definition_id(value: Any, context: str) -> str:
    asset_definition_id = _offline_exact_string(value, context)
    if _OFFLINE_ASSET_DEFINITION_ID_RE.fullmatch(asset_definition_id) is None:
        raise RuntimeError(
            f"{context} must be a canonical unprefixed Base58 asset definition id"
        )
    # Keep this complete validation synchronized with the normative Rust codec:
    # `iroha_data_model::asset::AssetDefinitionId::parse_address_literal`.
    payload = _decode_base_n(
        [BASE58_INDEX[symbol] for symbol in asset_definition_id],
        len(BASE58_ALPHABET),
    )
    uuid_bytes = payload[1:17]
    if (
        len(payload) != 21
        or payload[0] != 1
        or payload[17:] != blake3(payload[:17]).digest(length=4)
        or (uuid_bytes[6] >> 4) != 0b0100
        or (uuid_bytes[8] & 0b1100_0000) != 0b1000_0000
    ):
        raise RuntimeError(
            f"{context} must be a canonical checksummed UUIDv4 asset definition id"
        )
    return asset_definition_id

