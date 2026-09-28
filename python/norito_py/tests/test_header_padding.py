# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

import unittest

from norito.crc64 import crc64
from norito.errors import UnsupportedFeatureError
from norito.header import (
    COMPACT_LEN,
    MAX_HEADER_PADDING,
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

    def test_header_accepts_only_fixed_width_and_compact_length_layouts(self) -> None:
        payload = b"x"
        checksum = crc64(payload)
        for flags in range(256):
            framed = self._frame_with_unchecked_flags(payload, checksum, flags)
            header = NoritoHeader(
                schema_hash=b"\x00" * 16,
                payload_length=len(payload),
                checksum=checksum,
                flags=flags,
            )
            with self.subTest(flags=flags):
                if flags in (0, COMPACT_LEN):
                    decoded_header, decoded_payload = NoritoHeader.decode(framed)
                    self.assertEqual(decoded_header.flags, flags)
                    self.assertEqual(decoded_payload, payload)
                    self.assertEqual(header.encode()[-1], flags)
                else:
                    with self.assertRaises(UnsupportedFeatureError):
                        NoritoHeader.decode(framed)
                    with self.assertRaises(UnsupportedFeatureError):
                        header.encode()

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
