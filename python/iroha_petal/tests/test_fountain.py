# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Fountain code: masks, encoding and the incremental GF(2) decoder."""

from __future__ import annotations

import unittest

from petal_test_support import payload

from iroha_petal.fountain import (
    FountainDecoder,
    encode_atom,
    mask_len,
    mask_words,
    mix32,
    split_payload,
)
from iroha_petal.lanes import ATOM_LEN
from iroha_petal.prng import Xorshift32


def reassemble(atoms, length: int) -> bytes:
    return b"".join(atoms)[:length]


class FountainTest(unittest.TestCase):
    def test_systematic_atoms_alone_recover_the_payload(self) -> None:
        data = payload(100, 5)
        source = split_payload(data)
        decoder = FountainDecoder(len(source))
        for atom_id, atom in enumerate(source):
            self.assertTrue(decoder.add_encoded(7, atom_id, atom))
        self.assertEqual(reassemble(decoder.solve(), 100), data)

    def test_repair_atoms_cover_for_lost_systematic_atoms(self) -> None:
        data = payload(1000, 9)
        source = split_payload(data)
        k = len(source)
        crc = 0x12345678
        decoder = FountainDecoder(k)
        # lose every third systematic atom, then take repair atoms
        for atom_id in (i for i in range(k) if i % 3 != 0):
            decoder.add_encoded(crc, atom_id, encode_atom(source, crc, atom_id))
        atom_id = k
        used = 0
        while not decoder.is_complete:
            decoder.add_encoded(crc, atom_id, encode_atom(source, crc, atom_id))
            atom_id += 1
            used += 1
            self.assertLess(used, k, "decoder must converge")
        missing = sum(1 for i in range(k) if i % 3 == 0)
        self.assertLessEqual(used, missing + 8, f"needed {used} repairs for {missing} missing")
        self.assertEqual(reassemble(decoder.solve(), 1000), data)

    def test_pure_repair_streams_decode_with_small_overhead(self) -> None:
        data = payload(5000, 11)
        source = split_payload(data)
        k = len(source)
        crc = 0xCAFEF00D
        total_overhead = 0
        for trial in range(20):
            decoder = FountainDecoder(k)
            atom_id = k + trial * 1000
            received = 0
            while not decoder.is_complete:
                decoder.add_encoded(crc, atom_id, encode_atom(source, crc, atom_id))
                atom_id += 1
                received += 1
            total_overhead += received - k
            self.assertEqual(reassemble(decoder.solve(), 5000), data)
        self.assertLessEqual(total_overhead, 20 * 4, f"average overhead {total_overhead / 20}")

    def test_duplicate_and_dependent_atoms_do_not_raise_rank(self) -> None:
        source = split_payload(payload(60, 3))
        decoder = FountainDecoder(len(source))
        self.assertTrue(decoder.add_encoded(1, 0, source[0]))
        self.assertFalse(decoder.add_encoded(1, 0, source[0]))
        self.assertEqual(decoder.rank, 1)
        self.assertIsNone(decoder.solve())

    def test_repair_masks_span_far_more_than_thirty_two_dimensions(self) -> None:
        # Regression: an xorshift-derived mask is GF(2)-linear in a 32-bit seed
        # and can never exceed rank 32.
        k = 200
        decoder = FountainDecoder(k)
        for atom_id in range(k, k + 400):
            decoder.add(mask_words(k, 5, atom_id), bytes(ATOM_LEN))
        self.assertEqual(decoder.rank, k, "repair masks must reach full rank")

    def test_masks_are_nonzero_and_padded_bits_are_clear(self) -> None:
        for k in (1, 2, 31, 32, 33, 100):
            for atom_id in range(200):
                mask = mask_words(k, 99, atom_id)
                self.assertEqual(len(mask), mask_len(k))
                self.assertTrue(any(mask))
                self.assertTrue(all(0 <= word <= 0xFFFFFFFF for word in mask))
                if k % 32:
                    self.assertEqual(mask[-1] >> (k % 32), 0)

    def test_loss_and_reordering_still_recover_the_payload(self) -> None:
        data = payload(2_000, 31)
        source = split_payload(data)
        k = len(source)
        crc = 0x0BADF00D
        rng = Xorshift32(4242)
        ids = [i for i in range(3 * k) if rng.next_u32() % 10 >= 4]  # 40 % loss
        # Fisher-Yates shuffle: atoms arrive in any order
        for i in range(len(ids) - 1, 0, -1):
            j = rng.next_u32() % (i + 1)
            ids[i], ids[j] = ids[j], ids[i]
        decoder = FountainDecoder(k)
        used = 0
        for atom_id in ids:
            decoder.add_encoded(crc, atom_id, encode_atom(source, crc, atom_id))
            used += 1
            if decoder.is_complete:
                break
        self.assertTrue(decoder.is_complete)
        self.assertLessEqual(used, k + 12)
        self.assertEqual(reassemble(decoder.solve(), len(data)), data)

    def test_mix32_and_split_payload(self) -> None:
        self.assertEqual(mix32(0), 0)
        self.assertEqual(mix32(1), 1364076727)
        atoms = split_payload(bytes(range(20)))
        self.assertEqual(atoms, [bytes(range(16)), bytes(range(16, 20)) + bytes(12)])
        self.assertEqual(split_payload(b""), [])

    def test_decoder_rejects_malformed_input(self) -> None:
        with self.assertRaises(ValueError):
            FountainDecoder(0)
        with self.assertRaises(ValueError):
            mask_words(0, 1, 1)
        decoder = FountainDecoder(40)
        self.assertFalse(decoder.add([1], bytes(ATOM_LEN)), "wrong mask length")
        self.assertFalse(decoder.add([0, 1 << 9], bytes(ATOM_LEN)), "column past k")
        self.assertFalse(decoder.add([0, 0], bytes(ATOM_LEN)), "empty mask")
        with self.assertRaises(ValueError):
            decoder.add([1, 0], bytes(15))
        self.assertEqual(decoder.rank, 0)


if __name__ == "__main__":
    unittest.main()
