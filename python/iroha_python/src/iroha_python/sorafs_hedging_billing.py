"""Strict Norito codec for the V1 SoraFS billing acknowledgement proof."""

from __future__ import annotations

import hashlib
import re
from dataclasses import dataclass, field
from typing import Final

from norito.crc64 import crc64

SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1: Final[str] = (
    "iroha.torii.v1.sorafs.billing.acknowledgement_proof"
)
"""Stable shared Rust/Torii schema name for the acknowledgement proof."""

SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1: Final[int] = 64 * 1024
"""Maximum external authentication-proof size accepted by the V1 route."""

_SCHEMA_HASH: Final[bytes] = hashlib.sha256(
    b"norito:v1:type-name\0" + SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1.encode("ascii")
).digest()[:16]
_COMPACT_LENGTH_FLAG: Final[int] = 0x02


def _compact_length(value: int) -> bytes:
    encoded = bytearray()
    remaining = value
    while True:
        byte = remaining & 0x7F
        remaining >>= 7
        encoded.append(byte | (0x80 if remaining else 0))
        if not remaining:
            return bytes(encoded)


def _field(payload: bytes) -> bytes:
    return _compact_length(len(payload)) + payload


def _validate_request_nonce_hex(value: object) -> str:
    if type(value) is not str or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        raise TypeError(
            "request_nonce_hex must be one non-zero lowercase 32-byte hexadecimal digest"
        )
    if value == "0" * 64:
        raise ValueError(
            "request_nonce_hex must be one non-zero lowercase 32-byte hexadecimal digest"
        )
    return value


def _validate_authentication_proof(value: object) -> bytes:
    if type(value) is not bytes:
        raise TypeError("authentication_proof must be exact binary bytes")
    if not 1 <= len(value) <= SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1:
        raise ValueError(
            "authentication_proof must contain "
            f"1..={SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1} bytes"
        )
    return value


@dataclass(frozen=True)
class SorafsBillingAcknowledgementProofV1:
    """Exact two-field owner proof for one billing acknowledgement."""

    request_nonce_hex: str
    authentication_proof: bytes = field(repr=False)

    def __post_init__(self) -> None:
        _validate_request_nonce_hex(self.request_nonce_hex)
        _validate_authentication_proof(self.authentication_proof)

    def encode(self) -> bytes:
        """Encode the schema-bound, compact-length canonical Norito frame."""

        return encode_sorafs_billing_acknowledgement_proof_v1(
            self.request_nonce_hex,
            self.authentication_proof,
        )


def encode_sorafs_billing_acknowledgement_proof_v1(
    request_nonce_hex: str,
    authentication_proof: bytes,
) -> bytes:
    """Encode one exact shared V1 acknowledgement proof."""

    nonce = bytes.fromhex(_validate_request_nonce_hex(request_nonce_hex))
    proof = _validate_authentication_proof(authentication_proof)
    proof_vector = len(proof).to_bytes(8, "little") + proof
    payload = _field(nonce) + _field(proof_vector)
    header = (
        b"NRT0\x00\x00"
        + _SCHEMA_HASH
        + b"\x00"
        + len(payload).to_bytes(8, "little")
        + crc64(payload).to_bytes(8, "little")
        + bytes([_COMPACT_LENGTH_FLAG])
    )
    return header + payload


__all__ = [
    "SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1",
    "SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1",
    "SorafsBillingAcknowledgementProofV1",
    "encode_sorafs_billing_acknowledgement_proof_v1",
]
