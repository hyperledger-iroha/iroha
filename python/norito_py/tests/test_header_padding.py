# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

import unittest

from norito.crc64 import crc64
from norito.header import MAX_HEADER_PADDING, NoritoHeader


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


if __name__ == "__main__":
    unittest.main()
