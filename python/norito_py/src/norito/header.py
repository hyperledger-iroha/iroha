# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Norito header encoding/decoding."""

from __future__ import annotations

from dataclasses import dataclass
from typing import ClassVar

from .errors import (
    ChecksumMismatchError,
    InvalidMagicError,
    LengthMismatchError,
    SchemaMismatchError,
    UnsupportedCompressionError,
    UnsupportedFeatureError,
    UnsupportedVersionError,
)

MAGIC = b"NRT0"
# Norito major version is the ABI/protocol revision. Minor versions encode the
# compile-time default layout flags so that consumers can quickly detect which
# static features were enabled (packed sequences/structs, compact lengths).
MAJOR_VERSION = 0

# Header flag constants (mirroring `norito.md`)
PACKED_SEQ = 0x01
COMPACT_LEN = 0x02
PACKED_STRUCT = 0x04
VARINT_OFFSETS = 0x08
COMPACT_SEQ_LEN = 0x10
FIELD_BITSET = 0x20

# V1 minor version is fixed; layout flags are declared in the header flags byte.
MINOR_VERSION = 0

# Maximum allowed alignment padding between the header and payload.
MAX_HEADER_PADDING = 64

COMPRESSION_NONE = 0
COMPRESSION_ZSTD = 1

SUPPORTED_FLAGS = {
    PACKED_SEQ,
    COMPACT_LEN,
    PACKED_STRUCT,
    VARINT_OFFSETS,
    COMPACT_SEQ_LEN,
    FIELD_BITSET,
}

_SUPPORTED_FLAGS_MASK = 0
for _flag in SUPPORTED_FLAGS:
    _SUPPORTED_FLAGS_MASK |= _flag


@dataclass
class NoritoHeader:
    """Represents a Norito header."""

    schema_hash: bytes
    payload_length: int
    checksum: int
    flags: int
    compression: int = 0
    major: int = MAJOR_VERSION
    minor: int = MINOR_VERSION

    _HEADER_LEN: ClassVar[int] = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1

    def encode(self) -> bytes:
        """Serialize the header to bytes."""

        if len(self.schema_hash) != 16:
            raise ValueError("schema_hash must be 16 bytes long")
        if self.compression not in (COMPRESSION_NONE, COMPRESSION_ZSTD):
            raise UnsupportedCompressionError(self.compression)
        header = bytearray(self._HEADER_LEN)
        header[0:4] = MAGIC
        header[4] = self.major
        header[5] = self.minor
        header[6:22] = self.schema_hash
        header[22] = self.compression
        header[23:31] = self.payload_length.to_bytes(8, "little")
        header[31:39] = self.checksum.to_bytes(8, "little")
        header[39] = self.flags & 0xFF
        return bytes(header)

    @classmethod
    def decode(
        cls,
        data: bytes,
        *,
        expected_schema_hash: bytes | None = None,
        expected_flags: int | None = None,
    ) -> tuple["NoritoHeader", bytes]:
        """Parse a Norito header from the front of *data*.

        Returns a tuple ``(header, payload_bytes)``.
        """

        if len(data) < cls._HEADER_LEN:
            raise LengthMismatchError(cls._HEADER_LEN, len(data))
        if data[0:4] != MAGIC:
            raise InvalidMagicError("Invalid Norito magic")
        major = data[4]
        minor = data[5]
        if major != MAJOR_VERSION:
            raise UnsupportedVersionError(major, minor)
        schema_hash = data[6:22]
        compression = data[22]
        if compression not in (COMPRESSION_NONE, COMPRESSION_ZSTD):
            raise UnsupportedCompressionError(compression)
        payload_length = int.from_bytes(data[23:31], "little")
        checksum = int.from_bytes(data[31:39], "little")
        flags = data[39]

        if minor != MINOR_VERSION:
            raise UnsupportedVersionError(major, minor)

        if expected_schema_hash is not None and expected_schema_hash != schema_hash:
            raise SchemaMismatchError(expected_schema_hash, schema_hash)
        if expected_flags is not None and expected_flags != flags:
            raise SchemaMismatchError(
                expected_flags.to_bytes(1, "little"),
                flags.to_bytes(1, "little"),
            )

        unsupported = flags & ~_SUPPORTED_FLAGS_MASK
        if unsupported:
            raise UnsupportedFeatureError(unsupported)

        if compression == COMPRESSION_NONE:
            min_end = cls._HEADER_LEN + payload_length
            if len(data) < min_end:
                raise LengthMismatchError(min_end, len(data))
            padding_len = len(data) - min_end
            if padding_len > MAX_HEADER_PADDING:
                raise LengthMismatchError(min_end + MAX_HEADER_PADDING, len(data))
            if padding_len:
                padding = data[cls._HEADER_LEN : cls._HEADER_LEN + padding_len]
                if any(b != 0 for b in padding):
                    raise LengthMismatchError(payload_length, payload_length + len(padding))
            payload_start = cls._HEADER_LEN + padding_len
            payload_end = payload_start + payload_length
            payload = data[payload_start:payload_end]
            if len(payload) != payload_length:
                raise LengthMismatchError(payload_length, len(payload))
        else:
            payload = data[cls._HEADER_LEN :]
            if not payload:
                raise LengthMismatchError(payload_length, len(payload))

        header = cls(
            schema_hash=schema_hash,
            payload_length=payload_length,
            checksum=checksum,
            flags=flags,
            compression=compression,
            major=major,
            minor=minor,
        )
        return header, payload

    def validate_checksum(self, payload: bytes) -> None:
        from .crc64 import crc64

        actual = crc64(payload)
        if actual != self.checksum:
            raise ChecksumMismatchError(self.checksum, actual)


__all__ = [
    "NoritoHeader",
    "MAGIC",
    "MAJOR_VERSION",
    "MINOR_VERSION",
    "MAX_HEADER_PADDING",
    "PACKED_SEQ",
    "COMPACT_LEN",
    "PACKED_STRUCT",
    "VARINT_OFFSETS",
    "COMPACT_SEQ_LEN",
    "FIELD_BITSET",
    "COMPRESSION_NONE",
    "COMPRESSION_ZSTD",
]
