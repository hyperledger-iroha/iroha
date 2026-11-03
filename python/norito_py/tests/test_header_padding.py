# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

import unittest

from norito.crc64 import crc64
from norito.compression import decompress_zstd
from norito.errors import ChecksumMismatchError, UnsupportedFeatureError
from norito.header import (
    COMPACT_LEN,
    COMPRESSION_ZSTD,
    MAX_HEADER_PADDING,
    NoritoHeader,
)


class NoritoHeaderPaddingTests(unittest.TestCase):
    def test_compressed_frame_keeps_raw_length_checksum_and_layout(self) -> None:
        # A standard Zstd single-segment raw block containing the CRC64 check
        # vector. Native Metal/CUDA helper symbol names are not wire fields.
        compressed = bytes.fromhex("28b52ffd2009490000313233343536373839")
        fixed_header = bytes.fromhex(
            "4e52543000000000000000000000000000000000000001"
            "0900000000000000fa3919dfbbc95d9900"
        )
        for flags in (0, COMPACT_LEN):
            with self.subTest(flags=flags):
                expected_header = fixed_header[:-1] + bytes([flags])
                header, encoded_payload = NoritoHeader.decode(expected_header + compressed)
                self.assertEqual(header.encode(), expected_header)
                self.assertEqual(header.compression, COMPRESSION_ZSTD)
                self.assertEqual(header.payload_length, 9)
                self.assertEqual(header.flags, flags)
                self.assertEqual(encoded_payload, compressed)
                payload = decompress_zstd(encoded_payload, target_size=header.payload_length)
                self.assertEqual(payload, b"123456789")
                header.validate_checksum(payload)
                with self.assertRaises(ChecksumMismatchError):
                    header.validate_checksum(compressed)
                with self.assertRaises(ChecksumMismatchError):
                    header.validate_checksum(b"123456780")

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
