#!/usr/bin/env python3
# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0
"""Generate Android KeyDescription DER hex for mock attestation certificates."""

import argparse
import binascii


def der_len(length: int) -> bytes:
    if length < 0x80:
        return bytes([length])
    data = bytearray()
    while length:
        data.insert(0, length & 0xFF)
        length >>= 8
    return bytes([0x80 | len(data)]) + bytes(data)


def der_int(value: int, tag: int = 0x02) -> bytes:
    if value == 0:
        data = b"\x00"
    else:
        data = bytearray()
        tmp = value
        while tmp:
            data.insert(0, tmp & 0xFF)
            tmp >>= 8
        if data[0] & 0x80:
            data.insert(0, 0)
        data = bytes(data)
    return bytes([tag]) + der_len(len(data)) + data


def der_enum(value: int) -> bytes:
    return der_int(value, tag=0x0A)


def der_octet(data: bytes) -> bytes:
    return bytes([0x04]) + der_len(len(data)) + data


def der_sequence(content: bytes) -> bytes:
    return bytes([0x30]) + der_len(len(content)) + content


def build_key_description(challenge: bytes, unique: bytes) -> bytes:
    parts = []
    parts.append(der_int(200))
    parts.append(der_enum(2))
    parts.append(der_int(4))
    parts.append(der_enum(2))
    parts.append(der_octet(challenge))
    parts.append(der_octet(unique))
    parts.append(der_sequence(b""))
    parts.append(der_sequence(b""))
    parts.append(der_sequence(b""))
    return der_sequence(b"".join(parts))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("challenge", help="Hex-encoded attestation challenge (32 bytes).")
    parser.add_argument("unique", help="Hex-encoded unique ID (16 bytes).")
    args = parser.parse_args()

    challenge = binascii.unhexlify(args.challenge)
    unique = binascii.unhexlify(args.unique)
    der = build_key_description(challenge, unique)
    print(binascii.hexlify(der).decode())


if __name__ == "__main__":
    main()
