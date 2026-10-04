# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""CRC-32C, xorshift32, Reed-Solomon, layout, glyph and lane codec tests."""

from __future__ import annotations

import math
import struct
import unittest

from petal_test_support import Lcg

from iroha_petal import crc32c, layout
from iroha_petal.glyphs import (
    GLYPH_CHARS,
    GLYPH_COUNT,
    STROKES,
    TEMPLATES,
    generate_templates,
    is_inked,
)
from iroha_petal.lanes import (
    D_DATA,
    D_WORD,
    K_DATA,
    K_PARITY,
    K_WORD,
    P_DATA,
    P_WORD,
    FrameCells,
    Lane,
    decode_lane,
    decode_lane_counted,
    encode_lane,
)
from iroha_petal.layout import (
    CANVAS,
    CENTER,
    DOT_RADIUS,
    FINDER_CENTERS,
    FINDER_OUTER,
    MASK,
    RING_RADII,
    RING_SLOTS,
    TILE_COUNT,
    TILE_GRID,
    TILE_SIZE,
    TILES,
    SlotKind,
    _gate_slots,
    data_slots,
    finder_lit,
    slot_center,
    slot_roles,
    split_slot,
    tile_center,
)
from iroha_petal.prng import Xorshift32
from iroha_petal.rs import ReedSolomon, RsError, RsErrorKind


class CrcTest(unittest.TestCase):
    def test_matches_the_published_check_value(self) -> None:
        self.assertEqual(crc32c(b"123456789"), 0xE3069283)

    def test_empty_input_is_zero(self) -> None:
        self.assertEqual(crc32c(b""), 0)


class PrngTest(unittest.TestCase):
    def test_matches_the_reference_sequence(self) -> None:
        rng = Xorshift32(1)
        self.assertEqual(rng.next_u32(), 270_369)
        self.assertEqual(rng.next_u32(), 67_634_689)
        self.assertEqual(rng.next_u32(), 2_647_435_461)

    def test_zero_seed_is_remapped(self) -> None:
        self.assertEqual(Xorshift32(0), Xorshift32(0xDEADBEEF))

    def test_seed_must_be_32_bit(self) -> None:
        with self.assertRaises(ValueError):
            Xorshift32(1 << 32)
        with self.assertRaises(ValueError):
            Xorshift32(-1)

    def test_next_byte_is_the_top_byte(self) -> None:
        a, b = Xorshift32(77), Xorshift32(77)
        for _ in range(50):
            self.assertEqual(a.next_byte(), b.next_u32() >> 24)


class ReedSolomonTest(unittest.TestCase):
    def test_matches_the_qr_hello_world_check_vector(self) -> None:
        # QR Code version 1-M "HELLO WORLD": 16 data codewords, 10 EC codewords.
        data = bytes([32, 91, 11, 120, 209, 114, 220, 77, 67, 64, 236, 17, 236, 17, 236, 17])
        expected = bytes([196, 35, 39, 119, 235, 215, 231, 226, 93, 23])
        word = ReedSolomon(10).encode(data)
        self.assertEqual(word[:16], data)
        self.assertEqual(word[16:], expected)

    def test_corrects_random_errors_and_erasures_up_to_capacity(self) -> None:
        rng = Lcg(7)
        for k, nsym in ((16, 16), (60, 68), (12, 18), (13, 115)):
            rs = ReedSolomon(nsym)
            for _ in range(60):
                data = bytes(rng.byte() for _ in range(k))
                clean = rs.encode(data)
                n = len(clean)
                # pick f erasures and e errors with 2e + f <= nsym
                f = rng.below(min(nsym, n - 1) + 1)
                e = rng.below((nsym - f) // 2 + 1)
                word = bytearray(clean)
                positions = list(range(n))
                for i in range(f + e):
                    j = i + rng.below(n - i)
                    positions[i], positions[j] = positions[j], positions[i]
                erased = positions[:f]
                errored = positions[f : f + e]
                for p in erased:
                    word[p] = rng.byte()
                for p in errored:
                    word[p] ^= rng.byte() | 1
                corrected = rs.decode(word, erased)
                self.assertLessEqual(corrected, f + e)
                self.assertEqual(bytes(word), clean, f"k={k} nsym={nsym} f={f} e={e}")

    def test_rejects_words_beyond_capacity_without_returning_wrong_data(self) -> None:
        rng = Lcg(99)
        rs = ReedSolomon(16)
        wrong_accepts = 0
        for _ in range(200):
            data = bytes(rng.byte() for _ in range(16))
            clean = rs.encode(data)
            word = bytearray(clean)
            # 20 random errors is far beyond t = 8
            positions = list(range(len(word)))
            for i in range(20):
                j = i + rng.below(len(word) - i)
                positions[i], positions[j] = positions[j], positions[i]
            for p in positions[:20]:
                word[p] ^= rng.byte() | 1
            try:
                rs.decode(word, [])
            except RsError:
                continue
            # a miscorrection must at least be a valid codeword
            self.assertFalse(any(rs.syndromes(word)))
            if bytes(word) != clean:
                wrong_accepts += 1
        self.assertLessEqual(wrong_accepts, 2, f"miscorrection rate too high: {wrong_accepts}")

    def test_rejects_malformed_arguments(self) -> None:
        rs = ReedSolomon(4)
        with self.assertRaises(RsError) as caught:
            rs.decode(bytearray(4), [])
        self.assertIs(caught.exception.kind, RsErrorKind.INVALID_SHAPE)
        word = bytearray(rs.encode(bytes([1, 2, 3])))
        for erasures in ([9], [1, 1], [-1], [0, 1, 2, 3, 4]):
            with self.assertRaises(RsError) as caught:
                rs.decode(word, erasures)
            self.assertIs(caught.exception.kind, RsErrorKind.INVALID_SHAPE)
        with self.assertRaises(ValueError):
            ReedSolomon(0)
        with self.assertRaises(ValueError):
            ReedSolomon(255)
        with self.assertRaises(ValueError):
            ReedSolomon(10).encode(bytes(246))

    def test_clean_words_need_no_correction_and_failures_leave_the_word_alone(self) -> None:
        rs = ReedSolomon(6)
        word = bytearray(rs.encode(b"petal"))
        self.assertEqual(rs.decode(word), 0)
        damaged = bytearray(word)
        for i in range(5):
            damaged[i] ^= 0xFF
        before = bytes(damaged)
        with self.assertRaises(RsError) as caught:
            rs.decode(damaged)
        self.assertIs(caught.exception.kind, RsErrorKind.UNCORRECTABLE)
        self.assertEqual(bytes(damaged), before)


class LayoutTest(unittest.TestCase):
    def test_mask_has_256_symmetric_tiles(self) -> None:
        self.assertEqual(len(TILES), 256)
        for row in MASK:
            self.assertEqual(len(row), TILE_GRID)
            self.assertEqual(row, row[::-1], f"row {row} not mirrored")
        self.assertNotEqual(MASK[0], MASK[TILE_GRID - 1], "mask must show top from bottom")

    def test_tiles_are_row_major_and_inside_the_canvas(self) -> None:
        previous = None
        for index, (col, row) in enumerate(TILES):
            if previous is not None:
                self.assertGreater((row, col), (previous[1], previous[0]))
            previous = (col, row)
            x, y = tile_center(index)
            self.assertTrue(0.0 <= x < CANVAS and 0.0 <= y < CANVAS)

    def test_ring_slots_provide_exactly_the_lane_d_capacity(self) -> None:
        roles = slot_roles()
        kinds = [role.kind for role in roles]
        self.assertEqual(kinds.count(SlotKind.DATA), 240)
        self.assertEqual(kinds.count(SlotKind.GATE), 4 + 5 + 7)
        self.assertEqual(len(data_slots()), 240)
        # the two spare slots are the last non-reserved slots of the outer ring
        self.assertEqual(kinds.count(SlotKind.SPARE), 2)
        bits = sorted(role.bit for role in roles if role.kind is SlotKind.DATA)
        self.assertEqual(bits, list(range(240)))

    def test_gates_never_touch_the_top(self) -> None:
        for ring, n in enumerate(RING_SLOTS):
            gates, guards = _gate_slots(ring)
            for slot in gates + guards:
                top = 3 * n // 4
                self.assertGreater(abs(slot - top), 2, f"ring {ring} slot {slot} near the top")

    def test_finders_and_rings_do_not_overlap(self) -> None:
        outermost = RING_RADII[2] + DOT_RADIUS
        for x, y in FINDER_CENTERS:
            distance = math.hypot(x - CENTER, y - CENTER)
            self.assertGreater(distance - FINDER_OUTER, outermost + 20.0)
        farthest = 0.0
        h = TILE_SIZE / 2.0
        for index in range(TILE_COUNT):
            x, y = tile_center(index)
            for cx, cy in ((x - h, y - h), (x + h, y - h), (x - h, y + h), (x + h, y + h)):
                farthest = max(farthest, math.hypot(cx - CENTER, cy - CENTER))
        self.assertLess(farthest + 10.0, RING_RADII[0] - DOT_RADIUS)

    def test_split_slot_inverts_the_flat_index(self) -> None:
        flat = 0
        for ring, n in enumerate(RING_SLOTS):
            for slot in range(n):
                self.assertEqual(split_slot(flat), (ring, slot))
                flat += 1

    def test_slot_centres_are_single_precision_values_on_the_rings(self) -> None:
        # the angles use Rust's `f32::consts::TAU` (0x40C90FDB, 2 pi rounded to nearest),
        # not twice a single-precision pi (one ulp lower)
        self.assertEqual(struct.pack(">f", layout._TAU_F32).hex(), "40c90fdb")
        self.assertEqual(slot_center(0, 0), (872.0, 512.0))
        for ring, n in enumerate(RING_SLOTS):
            for slot in range(n):
                x, y = slot_center(ring, slot)
                for value in (x, y):
                    self.assertEqual(struct.unpack("<f", struct.pack("<f", value))[0], value)
                theta = math.tau * slot / n
                self.assertAlmostEqual(x, CENTER + RING_RADII[ring] * math.cos(theta), places=3)
                self.assertAlmostEqual(y, CENTER + RING_RADII[ring] * math.sin(theta), places=3)
        with self.assertRaises(IndexError):
            slot_center(0, 80)

    def test_finder_blossom_shape(self) -> None:
        self.assertTrue(finder_lit(0.0, 0.0))
        self.assertTrue(finder_lit(0.0, -34.0), "upper petal")
        self.assertFalse(finder_lit(0.0, -59.0), "notch at the upper petal tip")
        self.assertFalse(finder_lit(0.0, 59.0), "gap between the two lower petals")
        self.assertFalse(finder_lit(70.0, 0.0))


class GlyphTest(unittest.TestCase):
    def test_checked_in_templates_match_the_stroke_definitions(self) -> None:
        self.assertEqual(generate_templates(), TEMPLATES)

    def test_every_glyph_has_ink_inside_the_design_grid(self) -> None:
        self.assertEqual(len(GLYPH_CHARS), GLYPH_COUNT)
        for glyph, strokes in enumerate(STROKES):
            self.assertGreater(sum(TEMPLATES[glyph]), 255 * 6, f"glyph {glyph} has too little ink")
            for stroke in strokes:
                for x, y in stroke:
                    self.assertTrue(2.0 <= x <= 30.0 and 2.0 <= y <= 30.0)
                    self.assertTrue(is_inked(glyph, x, y))

    def test_glyphs_are_pairwise_distinct_under_blur(self) -> None:
        def feature(glyph: int) -> list:
            values = [float(c) for c in TEMPLATES[glyph]]
            mean = sum(values) / len(values)
            centered = [v - mean for v in values]
            norm = math.sqrt(sum(v * v for v in centered))
            return [v / norm for v in centered]

        features = [feature(g) for g in range(GLYPH_COUNT)]
        for a in range(GLYPH_COUNT):
            for b in range(a + 1, GLYPH_COUNT):
                dot = sum(x * y for x, y in zip(features[a], features[b]))
                self.assertGreater(1.0 - dot, 0.2, f"glyphs {a} and {b} are too similar")


class LaneTest(unittest.TestCase):
    def test_lane_sizes_are_consistent(self) -> None:
        self.assertEqual((P_WORD, K_WORD, D_WORD), (32, 128, 30))
        self.assertEqual((P_DATA, K_DATA, D_DATA), (19, 83, 19))
        for lane in Lane:
            self.assertEqual(lane.data_len + lane.parity_len, lane.word_len)

    def test_whitening_is_deterministic_and_balanced(self) -> None:
        for lane in Lane:
            a = lane.whitening()
            self.assertEqual(a, lane.whitening())
            ones = sum(bin(b).count("1") for b in a)
            bits = len(a) * 8
            self.assertTrue(
                bits * 38 // 100 < ones < bits * 62 // 100, f"{lane} ones {ones}/{bits}"
            )

    def test_lanes_roundtrip_through_cells(self) -> None:
        p_data = bytes(range(P_DATA))
        k_data = bytes((b * 37) & 0xFF for b in range(K_DATA))
        d_data = bytes(b ^ 0xA5 for b in range(D_DATA))
        p, k, d = (
            encode_lane(Lane.P, p_data),
            encode_lane(Lane.K, k_data),
            encode_lane(Lane.D, d_data),
        )
        cells = FrameCells.from_words(p, k, d)
        self.assertEqual(cells.p_word(), p)
        self.assertEqual(cells.k_word(), k)
        self.assertEqual(cells.d_word(), d)
        self.assertEqual(decode_lane(Lane.P, cells.p_word()), p_data)
        self.assertEqual(decode_lane(Lane.K, cells.k_word()), k_data)
        self.assertEqual(decode_lane(Lane.D, cells.d_word()), d_data)

    def test_all_zero_data_still_lights_roughly_half_the_cells(self) -> None:
        cells = FrameCells.from_words(
            encode_lane(Lane.P, bytes(P_DATA)),
            encode_lane(Lane.K, bytes(K_DATA)),
            encode_lane(Lane.D, bytes(D_DATA)),
        )
        lit = sum(cells.light)
        self.assertTrue(90 <= lit <= 166, f"{lit} light tiles")

    def test_gate_dots_are_always_lit_and_guards_dark(self) -> None:
        cells = FrameCells.from_words(b"\xff" * P_WORD, b"\xff" * K_WORD, b"\xff" * D_WORD)
        for slot, role in enumerate(slot_roles()):
            if role.kind in (SlotKind.GATE, SlotKind.DATA):
                self.assertTrue(cells.dots[slot])
            else:
                self.assertFalse(cells.dots[slot])

    def test_counted_decoding_reports_the_rewritten_positions(self) -> None:
        data = bytes(range(P_DATA))
        clean = encode_lane(Lane.P, data)
        self.assertEqual(decode_lane_counted(Lane.P, clean), (data, 0))
        damaged = bytearray(clean)
        for position in (0, 7, 19, 31):
            damaged[position] ^= 0xC3
        self.assertEqual(decode_lane_counted(Lane.P, bytes(damaged)), (data, 4))
        # erased positions count as rewritten even when they held the right byte
        self.assertEqual(decode_lane_counted(Lane.P, bytes(damaged), [0, 7, 19, 31, 3]), (data, 5))

    def test_decode_survives_burst_damage_to_a_lane(self) -> None:
        k_data = bytes(range(K_DATA))
        word = bytearray(encode_lane(Lane.K, k_data))
        for i in range(K_PARITY // 2):
            word[i] ^= 0x5A
        self.assertEqual(decode_lane(Lane.K, bytes(word)), k_data)

    def test_lane_codec_rejects_wrong_lengths(self) -> None:
        with self.assertRaises(ValueError):
            encode_lane(Lane.P, bytes(P_DATA + 1))
        with self.assertRaises(RsError) as caught:
            decode_lane(Lane.D, bytes(D_WORD - 1))
        self.assertIs(caught.exception.kind, RsErrorKind.INVALID_SHAPE)
        with self.assertRaises(ValueError):
            FrameCells.from_words(bytes(P_WORD), bytes(K_WORD), bytes(D_WORD + 1))
        with self.assertRaises(ValueError):
            FrameCells([False] * 256, [16] * 256, [False] * 276)


if __name__ == "__main__":
    unittest.main()
