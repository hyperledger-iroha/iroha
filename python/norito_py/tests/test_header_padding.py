# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

import unittest

from norito.crc64 import crc64
from norito.header import (
    COMPACT_LEN,
    COMPACT_SEQ_LEN,
    FIELD_BITSET,
    MAX_HEADER_PADDING,
    PACKED_STRUCT,
    VARINT_OFFSETS,
    NoritoHeader,
)


class NoritoHeaderPaddingTests(unittest.TestCase):
    def test_decode_accepts_zero_padding(self) -> None:
        payload = b"norito-padding"
        checksum = crc64(payload)
        header = NoritoHeader(
            schema_hash=b"\x00" * 16,
            payload_length=len(payload),
            checksum=checksum,
            flags=0,
        )
        padding = b"\x00" * 8
        framed = header.encode() + padding + payload
        decoded_header, decoded_payload = NoritoHeader.decode(framed)
        self.assertEqual(decoded_header.payload_length, len(payload))
        self.assertEqual(decoded_payload, payload)

    def test_decode_rejects_excess_padding(self) -> None:
        payload = b"x"
        checksum = crc64(payload)
        header = NoritoHeader(
            schema_hash=b"\x00" * 16,
            payload_length=len(payload),
            checksum=checksum,
            flags=0,
        )
        padding = b"\x00" * (MAX_HEADER_PADDING + 1)
        framed = header.encode() + padding + payload
        with self.assertRaises(Exception):
            NoritoHeader.decode(framed)

    def test_decode_rejects_reserved_flags(self) -> None:
        payload = b"x"
        checksum = crc64(payload)
        for flags in (VARINT_OFFSETS, COMPACT_SEQ_LEN, VARINT_OFFSETS | COMPACT_SEQ_LEN):
            header = self._frame_with_unchecked_flags(payload, checksum, flags)
            with self.subTest(flags=flags), self.assertRaises(Exception):
                NoritoHeader.decode(header)
            with self.subTest(flags=flags), self.assertRaises(Exception):
                NoritoHeader(
                    schema_hash=b"\x00" * 16,
                    payload_length=len(payload),
                    checksum=checksum,
                    flags=flags,
                ).encode()

    def test_decode_rejects_invalid_field_bitset_flags(self) -> None:
        payload = b"x"
        checksum = crc64(payload)
        for flags in (FIELD_BITSET, FIELD_BITSET | COMPACT_LEN, FIELD_BITSET | PACKED_STRUCT):
            header = self._frame_with_unchecked_flags(payload, checksum, flags)
            with self.subTest(flags=flags), self.assertRaises(Exception):
                NoritoHeader.decode(header)
            with self.subTest(flags=flags), self.assertRaises(Exception):
                NoritoHeader(
                    schema_hash=b"\x00" * 16,
                    payload_length=len(payload),
                    checksum=checksum,
                    flags=flags,
                ).encode()

    def _frame_with_unchecked_flags(self, payload: bytes, checksum: int, flags: int) -> bytes:
        header = NoritoHeader(
            schema_hash=b"\x00" * 16,
            payload_length=len(payload),
            checksum=checksum,
            flags=0,
        ).encode()
        return header[:-1] + bytes([flags & 0xFF]) + payload


if __name__ == "__main__":
    unittest.main()
