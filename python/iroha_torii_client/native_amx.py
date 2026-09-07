"""Canonical Native AMX V2 hashing and participant-identity helpers.

The routines in this module intentionally mirror the Rust data-model encodings
used by ``HashOf<Vec<PeerId>>``, lane proposal preimages, and nonrecursive Native participant
settlement commitments.  They are private SDK plumbing, not a second wire
format.
"""

from __future__ import annotations

import functools
import hashlib
import re
import struct
from typing import Any, Iterable, Mapping, Sequence, Tuple

_HASH_LITERAL_RE = re.compile(r"hash:([0-9A-F]{64})#[0-9A-F]{4}")
_BLS_NORMAL_PEER_ID_RE = re.compile(r"(?:bls_normal:)?(ea0130[0-9A-F]{96})")
_BLS12_381_BASE_FIELD = int(
    "1A0111EA397FE69A4B1BA7B6434BACD7"
    "64774B84F38512BF6730D2A0F6B0F624"
    "1EABFFFEB153FFFFB9FEFFFFFFFFAAAB",
    16,
)
_BLS12_381_SCALAR_FIELD = int(
    "73EDA753299D7D483339D80809A1D805"
    "53BDA402FFFE5BFEFFFFFFFF00000001",
    16,
)
_CRC64_XZ_POLYNOMIAL = 0xC96C5795D7870F42
_U64_MASK = (1 << 64) - 1

_DESCRIPTOR_PREIMAGE_TYPE = (
    "iroha_data_model::block::consensus::LaneBlockDescriptorPreimage"
)
_PROPOSAL_PREIMAGE_TYPE = (
    "iroha_data_model::block::consensus::LaneBlockProposalPreimage"
)
_SETTLEMENT_TYPE = "iroha_data_model::block::consensus::NativeAmxParticipantSettlement"
_SETTLEMENT_HASH_DOMAIN = b"iroha:native-amx:participant-settlement:v1"
_APPLICATION_MANIFEST_LEAF_DOMAIN = b"iroha:merkle:leaf:v1\0"


def _crc16_ccitt_false(payload: bytes) -> int:
    crc = 0xFFFF
    for byte in payload:
        crc ^= byte << 8
        for _ in range(8):
            crc = (
                ((crc << 1) ^ 0x1021) & 0xFFFF
                if crc & 0x8000
                else (crc << 1) & 0xFFFF
            )
    return crc


def _hash_bytes(payload: bytes) -> bytes:
    digest = bytearray(hashlib.blake2b(payload, digest_size=32).digest())
    digest[-1] |= 1
    return bytes(digest)


def _hash_literal(payload: bytes) -> str:
    body = _hash_bytes(payload).hex().upper()
    checksum = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    return f"hash:{body}#{checksum:04X}"


def _hash_literal_bytes(value: str) -> bytes:
    matched = _HASH_LITERAL_RE.fullmatch(value)
    if matched is None:
        raise ValueError("expected a previously validated canonical hash literal")
    return bytes.fromhex(matched.group(1))


def compute_native_amx_application_manifest_singleton_root(leaf_hash: str) -> str:
    """Derive the exact singleton Native AMX application-manifest root."""

    leaf_hash_bytes = _hash_literal_bytes(leaf_hash)
    body = leaf_hash_bytes.hex().upper()
    checksum = _crc16_ccitt_false(f"hash:{body}".encode("ascii"))
    canonical = f"hash:{body}#{checksum:04X}"
    if leaf_hash != canonical or leaf_hash_bytes[-1] & 1 != 1:
        raise ValueError("manifest leaf hash must be canonical")
    return _hash_literal(_APPLICATION_MANIFEST_LEAF_DOMAIN + leaf_hash_bytes)


def _u8(value: int) -> bytes:
    return struct.pack("<B", value)


def _u16(value: int) -> bytes:
    return struct.pack("<H", value)


def _u32(value: int) -> bytes:
    return struct.pack("<I", value)


def _u64(value: int) -> bytes:
    return struct.pack("<Q", value)


def _unsigned_leb128(value: int) -> bytes:
    if value < 0:
        raise ValueError("compact lengths must be non-negative")
    encoded = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        if remaining:
            byte |= 0x80
        encoded.append(byte)
        if not remaining:
            return bytes(encoded)


def _field(payload: bytes) -> bytes:
    return _unsigned_leb128(len(payload)) + payload


def _struct(fields: Iterable[bytes]) -> bytes:
    return b"".join(_field(field) for field in fields)


def _string(value: str) -> bytes:
    encoded = value.encode("utf-8")
    return _unsigned_leb128(len(encoded)) + encoded


def _vector(items: Sequence[Any], encoder: Any) -> bytes:
    encoded = bytearray(_u64(len(items)))
    for item in items:
        encoded.extend(_field(encoder(item)))
    return bytes(encoded)


def _lane_id(value: int) -> bytes:
    return _field(_u32(value))


def _dataspace_id(value: int) -> bytes:
    return _field(_u64(value))


def _optional_hash(value: str | None) -> bytes:
    if value is None:
        return b"\x00"
    return b"\x01" + _field(_hash_literal_bytes(value))


def _crc64_xz(payload: bytes) -> int:
    crc = _U64_MASK
    for byte in payload:
        crc ^= byte
        for _ in range(8):
            crc = (
                (crc >> 1) ^ _CRC64_XZ_POLYNOMIAL
                if crc & 1
                else crc >> 1
            )
    return crc ^ _U64_MASK


def _norito_frame(type_name: str, payload: bytes) -> bytes:
    schema = hashlib.sha256(
        b"norito:v1:type-name\0" + type_name.encode("utf-8")
    ).digest()[:16]
    return b"".join(
        (
            b"NRT0",
            b"\x00\x00",
            schema,
            b"\x00",
            _u64(len(payload)),
            _u64(_crc64_xz(payload)),
            b"\x02",
            payload,
        )
    )


def _jacobian_double(
    point: Tuple[int, int, int],
) -> Tuple[int, int, int]:
    x, y, z = point
    if z == 0 or y == 0:
        return (0, 1, 0)
    modulus = _BLS12_381_BASE_FIELD
    a = x * x % modulus
    b = y * y % modulus
    c = b * b % modulus
    d = 2 * ((x + b) * (x + b) - a - c) % modulus
    e = 3 * a % modulus
    f = e * e % modulus
    return (
        (f - 2 * d) % modulus,
        (e * (d - (f - 2 * d)) - 8 * c) % modulus,
        2 * y * z % modulus,
    )


def _jacobian_add_affine(
    point: Tuple[int, int, int],
    affine: Tuple[int, int],
) -> Tuple[int, int, int]:
    x1, y1, z1 = point
    x2, y2 = affine
    if z1 == 0:
        return (x2, y2, 1)
    modulus = _BLS12_381_BASE_FIELD
    z1_squared = z1 * z1 % modulus
    u2 = x2 * z1_squared % modulus
    s2 = y2 * z1_squared * z1 % modulus
    h = (u2 - x1) % modulus
    if h == 0:
        return _jacobian_double(point) if s2 == y1 else (0, 1, 0)
    hh = h * h % modulus
    i = 4 * hh % modulus
    j = h * i % modulus
    r = 2 * (s2 - y1) % modulus
    v = x1 * i % modulus
    x3 = (r * r - j - 2 * v) % modulus
    y3 = (r * (v - x3) - 2 * y1 * j) % modulus
    z3 = ((z1 + h) * (z1 + h) - z1_squared - hh) % modulus
    return (x3, y3, z3)


def _is_in_bls12_381_g1_subgroup(point: Tuple[int, int]) -> bool:
    result = (0, 1, 0)
    for bit in bin(_BLS12_381_SCALAR_FIELD)[2:]:
        result = _jacobian_double(result)
        if bit == "1":
            result = _jacobian_add_affine(result, point)
    return result[2] == 0


@functools.lru_cache(maxsize=512)
def _decode_bls_normal_peer_id_core(bare: str) -> Tuple[str, bytes]:
    compressed = bytes.fromhex(bare[6:])
    first = compressed[0]
    compressed_flag = bool(first & 0x80)
    infinity_flag = bool(first & 0x40)
    sign_flag = bool(first & 0x20)
    x = int.from_bytes(bytes([first & 0x1F]) + compressed[1:], "big")
    modulus = _BLS12_381_BASE_FIELD
    if not compressed_flag or infinity_flag or x >= modulus:
        raise ValueError("contains an invalid BLS-Normal public key")
    rhs = (pow(x, 3, modulus) + 4) % modulus
    y = pow(rhs, (modulus + 1) // 4, modulus)
    if y * y % modulus != rhs:
        raise ValueError("contains an invalid BLS-Normal public key")
    if (y * 2 > modulus) != sign_flag:
        y = modulus - y
    if not _is_in_bls12_381_g1_subgroup((x, y)):
        raise ValueError("contains a non-subgroup BLS-Normal public key")
    return bare, b"\x02" + compressed


def decode_bls_normal_peer_id(value: str, context: str = "validator") -> Tuple[str, bytes]:
    """Decode one canonical BLS-Normal ``PeerId``.

    The returned bytes are the public-key compact encoding used by Rust's
    ``PeerId`` ordering and Norito codec.
    """

    if not isinstance(value, str):
        raise TypeError(f"{context} must be a canonical BLS-Normal PeerId string")
    if value.strip() != value:
        raise ValueError(f"{context} must not contain surrounding whitespace")
    matched = _BLS_NORMAL_PEER_ID_RE.fullmatch(value)
    if matched is None:
        raise ValueError(f"{context} must be a canonical BLS-Normal PeerId")
    try:
        return _decode_bls_normal_peer_id_core(matched.group(1))
    except ValueError as exc:
        raise ValueError(f"{context} {exc}") from exc


def validate_bls_normal_validator_set(
    validators: Sequence[str], context: str
) -> Tuple[str, ...]:
    """Validate and canonicalize an ordered Native AMX validator set."""

    canonical = []
    ordering_keys = []
    for index, validator in enumerate(validators):
        peer, key = decode_bls_normal_peer_id(
            validator, f"{context}[{index}]"
        )
        canonical.append(peer)
        ordering_keys.append(key)
    if any(
        left >= right
        for left, right in zip(ordering_keys, ordering_keys[1:])
    ):
        raise ValueError(
            f"{context} must be strictly ordered by canonical validator id"
        )
    return tuple(canonical)


def _peer_id(value: str) -> bytes:
    _, public_key = decode_bls_normal_peer_id(value)
    compact_key = _u64(len(public_key)) + b"".join(
        _field(bytes((byte,))) for byte in public_key
    )
    return _field(compact_key)


def _validator_vector(validators: Sequence[str]) -> bytes:
    return _vector(validators, _peer_id)


def compute_native_amx_validator_set_hash(validators: Sequence[str]) -> str:
    """Recompute Rust ``HashOf<Vec<PeerId>>`` for an ordered committee."""

    return _hash_literal(_validator_vector(validators))


def compute_native_amx_descriptor_hash(descriptor: Mapping[str, Any]) -> str:
    """Recompute ``LaneBlockDescriptorV1::computed_descriptor_hash``."""

    payload = _struct(
        (
            _string("nexus:lane-block-descriptor:v1"),
            _u8(1),
            _lane_id(descriptor["lane_id"]),
            _dataspace_id(descriptor["dataspace_id"]),
            _hash_literal_bytes(descriptor["lane_incarnation"]),
            _u64(descriptor["proposal_height"]),
            _u64(descriptor["previous_lane_block_height"]),
            _optional_hash(descriptor.get("previous_lane_block_descriptor_hash")),
            _u64(descriptor["lane_block_height"]),
            _u64(descriptor["lane_block_view"]),
            _hash_literal_bytes(descriptor["subject_hash"]),
            _hash_literal_bytes(descriptor["payload_ownership_hash"]),
            _hash_literal_bytes(descriptor["rbc_instance_hash"]),
            _vector(descriptor["accepted_candidate_indices"], _u64),
            _vector(
                descriptor["accepted_transaction_hashes"], _hash_literal_bytes
            ),
            _u16(descriptor["validator_set_hash_version"]),
            _hash_literal_bytes(descriptor["validator_set_hash"]),
            _validator_vector(descriptor["validator_set"]),
            _u32(descriptor["validator_count"]),
            _u32(descriptor["min_quorum"]),
            _string(descriptor["qc_mode_tag"]),
        )
    )
    return _hash_literal(_norito_frame(_DESCRIPTOR_PREIMAGE_TYPE, payload))


def compute_native_amx_proposal_hash(descriptor: Mapping[str, Any]) -> str:
    """Recompute ``LaneBlockProposalV1::computed_proposal_hash``."""

    payload = _struct(
        (
            _string("nexus:lane-block-proposal:v1"),
            _u8(1),
            _u64(descriptor["proposal_height"]),
            _hash_literal_bytes(descriptor["descriptor_hash"]),
            _lane_id(descriptor["lane_id"]),
            _dataspace_id(descriptor["dataspace_id"]),
            _hash_literal_bytes(descriptor["lane_incarnation"]),
            _u64(descriptor["lane_block_height"]),
            _u64(descriptor["lane_block_view"]),
            _hash_literal_bytes(descriptor["subject_hash"]),
            _hash_literal_bytes(descriptor["payload_ownership_hash"]),
            _hash_literal_bytes(descriptor["rbc_instance_hash"]),
            _vector(descriptor["accepted_candidate_indices"], _u64),
            _vector(
                descriptor["accepted_transaction_hashes"], _hash_literal_bytes
            ),
            _u16(descriptor["validator_set_hash_version"]),
            _hash_literal_bytes(descriptor["validator_set_hash"]),
            _validator_vector(descriptor["validator_set"]),
            _u32(descriptor["validator_count"]),
            _u32(descriptor["min_quorum"]),
            _string(descriptor["qc_mode_tag"]),
        )
    )
    return _hash_literal(_norito_frame(_PROPOSAL_PREIMAGE_TYPE, payload))


def parse_native_amx_participant_settlement(value: Any) -> dict[str, Any]:
    """Validate the sole seven-field Native AMX participant settlement schema.

    Source IDs retain candidate order. Economic and nested settlement fields
    are unknown fields and are rejected even when empty or zero.
    """
    fields = (
        "lane_id", "dataspace_id", "lane_incarnation",
        "participant_lane_block_height", "authority_context_height",
        "previous_native_settlement_hash", "source_ids",
    )
    if not isinstance(value, Mapping):
        raise TypeError("Native AMX participant settlement must be an object")
    if set(value) != set(fields):
        raise ValueError("Native AMX participant settlement requires exactly its seven fields")
    parsed: dict[str, Any] = {}
    for field, bits, positive in (
        ("lane_id", 32, False), ("dataspace_id", 64, False),
        ("participant_lane_block_height", 64, True), ("authority_context_height", 64, True),
    ):
        integer = value[field]
        if isinstance(integer, bool) or not isinstance(integer, int):
            raise TypeError(f"Native AMX participant settlement {field} must be an unsigned integer")
        if not (int(positive) <= integer < 1 << bits):
            raise ValueError(f"Native AMX participant settlement {field} is outside its protocol bound")
        parsed[field] = integer
    incarnation = value["lane_incarnation"]
    if not isinstance(incarnation, str) or _HASH_LITERAL_RE.fullmatch(incarnation) is None:
        raise ValueError("Native AMX participant settlement incarnation must be a canonical hash")
    body = _hash_literal_bytes(incarnation)
    checksum = _crc16_ccitt_false(incarnation[:69].encode("ascii"))
    if body == bytes(31) + b"\x01" or not any(body) or body[-1] & 1 == 0 or int(incarnation[70:], 16) != checksum:
        raise ValueError("Native AMX participant settlement incarnation is not a valid nonzero hash")
    previous = value["previous_native_settlement_hash"]
    if previous is not None:
        if not isinstance(previous, str) or _HASH_LITERAL_RE.fullmatch(previous) is None:
            raise ValueError("Native AMX previous settlement hash must be canonical or null")
        previous_body = _hash_literal_bytes(previous)
        previous_checksum = _crc16_ccitt_false(previous[:69].encode("ascii"))
        if (previous_body == bytes(31) + b"\x01" or not any(previous_body)
                or previous_body[-1] & 1 == 0 or int(previous[70:], 16) != previous_checksum):
            raise ValueError("Native AMX previous settlement hash must be nonzero and canonical")
        if parsed["participant_lane_block_height"] == 1:
            raise ValueError("Native AMX previous settlement hash must be null at participant height one")
    sources = value["source_ids"]
    if not isinstance(sources, (list, tuple)) or not 1 <= len(sources) <= 4096:
        raise ValueError("Native AMX participant settlement source_ids must contain 1..4096 sources")
    for source in sources:
        if not isinstance(source, str) or re.fullmatch(r"[0-9A-F]{64}", source) is None:
            raise ValueError("Native AMX participant settlement source_ids must be canonical uppercase 32-byte hex")
        if source == "00" * 32:
            raise ValueError("Native AMX participant settlement source_ids must be nonzero")
    if len(set(sources)) != len(sources):
        raise ValueError("Native AMX participant settlement source_ids must be unique")
    return {
        "lane_id": parsed["lane_id"],
        "dataspace_id": parsed["dataspace_id"],
        "lane_incarnation": incarnation,
        "participant_lane_block_height": parsed["participant_lane_block_height"],
        "authority_context_height": parsed["authority_context_height"],
        "previous_native_settlement_hash": previous,
        "source_ids": list(sources),
    }


def compute_native_amx_participant_settlement_hash(settlement: Mapping[str, Any]) -> str:
    """Hash the validated nonrecursive participant settlement exactly as Rust."""
    settlement = parse_native_amx_participant_settlement(settlement)
    payload = _struct((
        _lane_id(settlement["lane_id"]),
        _dataspace_id(settlement["dataspace_id"]),
        _hash_literal_bytes(settlement["lane_incarnation"]),
        _u64(settlement["participant_lane_block_height"]),
        _u64(settlement["authority_context_height"]),
        _optional_hash(settlement["previous_native_settlement_hash"]),
        # Norito [u8; 32] frames each byte, unlike the raw Hash representation.
        _vector(settlement["source_ids"], lambda source:
                _struct(bytes([byte]) for byte in bytes.fromhex(source))),
    ))
    frame = _norito_frame(_SETTLEMENT_TYPE, payload)
    return _hash_literal(_u64(len(_SETTLEMENT_HASH_DOMAIN)) + _SETTLEMENT_HASH_DOMAIN + frame)
