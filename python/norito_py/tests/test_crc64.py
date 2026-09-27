# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Tests for the Norito CRC64 implementation."""

from norito.crc64 import crc64, crc64_concat


def test_crc64_empty_payload() -> None:
    assert crc64(b"") == 0


def test_crc64_known_vector() -> None:
    assert crc64(b"123456789") == 0x995DC9BBDF1939FA


def test_crc64_concat_matches_single_payload() -> None:
    parts = (b"hello", b" ", b"world")
    assert crc64_concat(parts) == crc64(b"".join(parts))
