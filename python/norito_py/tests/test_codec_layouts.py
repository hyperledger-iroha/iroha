# Copyright 2024 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

import unittest

from norito import (
    SchemaDescriptor,
    StructAdapter,
    StructField,
    decode,
    encode,
    seq,
    string,
    u16,
    u32,
)
from norito.header import COMPACT_LEN, NoritoHeader

_SCHEMA = SchemaDescriptor.type_name("norito_py::tests::Layout")


class NoritoCodecLayoutTests(unittest.TestCase):
    def test_sequence_writes_u64_count_then_elements(self) -> None:
        adapter = seq(u16())
        for flags in (0, COMPACT_LEN):
            with self.subTest(flags=flags):
                framed = encode([1, 2], _SCHEMA, adapter, flags=flags)
                _, payload = NoritoHeader.decode(framed)
                self.assertEqual(payload, (2).to_bytes(8, "little") + b"\x01\x00\x02\x00")
                self.assertEqual(decode(framed, adapter, schema=_SCHEMA), [1, 2])

    def test_struct_writes_fields_in_declaration_order(self) -> None:
        adapter = StructAdapter(
            [StructField("id", u32()), StructField("name", string())],
        )
        value = {"id": 7, "name": "ab"}
        expected = {
            0: b"\x07\x00\x00\x00" + (2).to_bytes(8, "little") + b"ab",
            COMPACT_LEN: b"\x07\x00\x00\x00" + b"\x02" + b"ab",
        }
        for flags, expected_payload in expected.items():
            with self.subTest(flags=flags):
                framed = encode(value, _SCHEMA, adapter, flags=flags)
                _, payload = NoritoHeader.decode(framed)
                self.assertEqual(payload, expected_payload)
                self.assertEqual(decode(framed, adapter, schema=_SCHEMA), value)


if __name__ == "__main__":
    unittest.main()
