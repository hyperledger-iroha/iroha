# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""CRC-32C, xorshift32, Reed-Solomon, layout and lane codec tests."""

from __future__ import annotations

import math
import struct
import unittest

from petal_test_support import Lcg

from iroha_petal import crc32c, layout
from iroha_petal.lanes import (
    ATOMS_PER_FRAME,
    D_DATA,
    D_WORD,
    DECODE_ORDER,
    FIELD_DATA,
    FIELD_LANES,
    FIELD_PARITY,
    FIELD_WORD,
    SPARE_SECTOR_CELLS,
    FrameCells,
    Lane,
    decode_lane,
    decode_lane_counted,
    encode_lane,
)
from iroha_petal.layout import (
    BAND_COUNT,
    BAND_OFFSETS,
    BAND_SLOTS,
    CENTER,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    FINDER_OUTER,
    MASK,
    MASK_GRID,
    PETAL_HALF_LENGTH,
    PETAL_HALF_WIDTH,
    PETAL_NOTCH_DISTANCE,
    PETAL_NOTCH_RADIUS,
    PETAL_RADIUS,
    RING_RADII,
    RING_SLOTS,
    SECTOR_CELLS,
    SECTORS,
    TOTAL_SLOTS,
    SlotKind,
    _gate_slots,
    band_radius,
    cell_angle,
    cell_band_slot,
    cell_center,
    cell_light,
    data_slots,
    finder_lit,
    in_mask,
    petal_lit,
    sector_cells,
    slot_center,
    slot_roles,
    split_slot,
)
from iroha_petal.prng import Xorshift32
from iroha_petal.rs import ReedSolomon, RsError, RsErrorKind


def field_data(seed: int) -> bytes:
    """The field lane data of the reference lane tests."""
    return bytes(((b * 37) & 0xFF) ^ seed for b in range(FIELD_DATA))


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
    def test_mask_is_mirror_symmetric_and_shows_which_way_is_up(self) -> None:
        for row in MASK:
            self.assertEqual(len(row), MASK_GRID)
            self.assertEqual(row, row[::-1], f"row {row} not mirrored")
        self.assertNotEqual(MASK[0], MASK[MASK_GRID - 1], "mask must show top from bottom")

    def test_bands_hold_every_cell_and_split_into_equal_sectors(self) -> None:
        self.assertEqual(BAND_OFFSETS[BAND_COUNT], FIELD_CELLS)
        seen = [False] * FIELD_CELLS
        for sector in range(SECTORS):
            cells = sector_cells(sector)
            self.assertEqual(len(cells), SECTOR_CELLS)
            for cell in cells:
                self.assertFalse(seen[cell], f"cell {cell} in two sectors")
                seen[cell] = True
                # a cell sits in its own quarter turn (odd bands lean half a slot forward)
                band, slot = cell_band_slot(cell)
                quarter = BAND_SLOTS[band] // SECTORS
                self.assertEqual(slot // quarter, sector)
                turn = cell_angle(cell) / math.tau * SECTORS
                self.assertTrue(sector - 1e-9 < turn < sector + 1.0)
            # bit order: band by band from the centre out, slots ascending
            self.assertEqual(list(cells), sorted(cells))
        self.assertTrue(all(seen))

    def test_band_slots_follow_the_pitch_rule(self) -> None:
        for band, slots in enumerate(BAND_SLOTS):
            raw = math.tau * band_radius(band) / 50.0
            self.assertEqual(slots, 4 * max(int(math.floor(raw + 0.5)), 1), band)

    def test_the_field_maps_onto_itself_under_a_quarter_turn(self) -> None:
        for cell in range(FIELD_CELLS):
            band, slot = cell_band_slot(cell)
            n = BAND_SLOTS[band]
            turned = BAND_OFFSETS[band] + (slot + n // 4) % n
            x, y = cell_center(cell)
            tx, ty = cell_center(turned)
            # a quarter turn clockwise on screen about the centre
            self.assertLess(abs(tx - (CENTER - (y - CENTER))), 1e-9)
            self.assertLess(abs(ty - (CENTER + (x - CENTER))), 1e-9)

    def test_petals_stay_clear_of_their_neighbours_and_the_rings(self) -> None:
        outer = band_radius(BAND_COUNT - 1) + PETAL_HALF_LENGTH
        self.assertLess(outer + 10.0, RING_RADII[0] - DOT_RADIUS)
        for cell in range(FIELD_CELLS):
            x, y = cell_center(cell)
            band, _ = cell_band_slot(cell)
            for other in range(
                BAND_OFFSETS[max(band - 1, 0)], BAND_OFFSETS[min(band + 2, BAND_COUNT)]
            ):
                if other == cell:
                    continue
                ox, oy = cell_center(other)
                distance = math.sqrt((x - ox) * (x - ox) + (y - oy) * (y - oy))
                self.assertGreater(
                    distance, 2.0 * PETAL_RADIUS, f"cells {cell} and {other} are {distance} apart"
                )

    def test_the_light_cells_draw_the_character(self) -> None:
        light = sum(1 for cell in range(FIELD_CELLS) if cell_light(cell))
        self.assertTrue(1200 <= light < 2400, f"{light} light cells")
        self.assertTrue(in_mask(512.0, 190.0), "the top bar")
        self.assertFalse(in_mask(512.0, 350.0), "the gap under the second bar")
        self.assertFalse(in_mask(181.9, 300.0))
        self.assertFalse(in_mask(300.0, 182.0 + 20 * 33.0))
        for cell in (0, 1000, FIELD_CELLS - 1):
            self.assertEqual(cell_light(cell), in_mask(*cell_center(cell)))

    def test_ring_slots_provide_exactly_the_lane_d_capacity(self) -> None:
        roles = slot_roles()
        kinds = [role.kind for role in roles]
        self.assertEqual(len(roles), TOTAL_SLOTS)
        self.assertEqual(kinds.count(SlotKind.DATA), 240)
        self.assertEqual(kinds.count(SlotKind.GATE), 4 + 5 + 7)
        self.assertEqual(len(data_slots()), 240)
        self.assertEqual(kinds.count(SlotKind.SPARE), TOTAL_SLOTS - 240 - 16 - 18)
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
        self.assertEqual(slot_center(0, 0), (917.0, 512.0))
        for ring, n in enumerate(RING_SLOTS):
            for slot in range(n):
                x, y = slot_center(ring, slot)
                for value in (x, y):
                    self.assertEqual(struct.unpack("<f", struct.pack("<f", value))[0], value)
                theta = math.tau * slot / n
                self.assertAlmostEqual(x, CENTER + RING_RADII[ring] * math.cos(theta), places=3)
                self.assertAlmostEqual(y, CENTER + RING_RADII[ring] * math.sin(theta), places=3)
        with self.assertRaises(IndexError):
            slot_center(0, 88)

    def test_petal_constants_are_the_reference_single_precision_values(self) -> None:
        self.assertEqual(PETAL_RADIUS, 4.599999904632568)
        self.assertEqual(PETAL_HALF_LENGTH, 5.199999809265137)
        self.assertEqual(PETAL_HALF_WIDTH, 4.300000190734863)
        self.assertEqual(PETAL_NOTCH_RADIUS, 1.2999999523162842)
        self.assertEqual(PETAL_NOTCH_DISTANCE, 5.400000095367432)

    def test_finder_blossom_shape(self) -> None:
        self.assertTrue(finder_lit(0.0, 0.0))
        self.assertTrue(finder_lit(0.0, -34.0), "upper petal")
        self.assertFalse(finder_lit(0.0, -59.0), "notch at the upper petal tip")
        self.assertFalse(finder_lit(0.0, 59.0), "gap between the two lower petals")
        self.assertFalse(finder_lit(70.0, 0.0))

    def test_a_petal_is_lit_at_its_centre_and_notched_at_its_tip(self) -> None:
        self.assertTrue(petal_lit(0.0, 0.0, 0.3))
        tip = PETAL_NOTCH_DISTANCE
        self.assertFalse(petal_lit(tip * math.cos(0.3), tip * math.sin(0.3), 0.3))
        self.assertFalse(petal_lit(0.0, 10.0, 0.0))
        # longer along its outward direction than across it
        self.assertTrue(petal_lit(-5.0, 0.0, 0.0))
        self.assertFalse(petal_lit(0.0, -5.0, 0.0))
        self.assertTrue(petal_lit(0.0, -5.0, math.pi / 2.0))

    def test_cell_lookups_validate_their_index(self) -> None:
        self.assertEqual(cell_band_slot(0), (0, 0))
        self.assertEqual(cell_band_slot(8), (1, 0))
        self.assertEqual(cell_band_slot(FIELD_CELLS - 1), (BAND_COUNT - 1, 191))
        for bad in (-1, FIELD_CELLS):
            with self.assertRaises(IndexError):
                cell_center(bad)
            with self.assertRaises(IndexError):
                cell_light(bad)
        with self.assertRaises(IndexError):
            sector_cells(4)


class LaneTest(unittest.TestCase):
    def test_lane_sizes_are_consistent(self) -> None:
        self.assertEqual(D_WORD, 30)
        self.assertEqual((D_DATA, FIELD_DATA), (19, 83))
        self.assertEqual(SPARE_SECTOR_CELLS, 4)
        self.assertEqual(ATOMS_PER_FRAME, 21)
        self.assertEqual(DECODE_ORDER, (Lane.D, Lane.F0, Lane.F1, Lane.F2, Lane.F3))
        self.assertEqual(tuple(Lane), DECODE_ORDER)
        for lane in Lane:
            self.assertEqual(lane.data_len + lane.parity_len, lane.word_len)
        self.assertEqual([lane.letter for lane in Lane], ["D", "0", "1", "2", "3"])
        self.assertEqual([lane.sector for lane in Lane], [None, 0, 1, 2, 3])
        self.assertEqual([Lane.field(s) for s in range(SECTORS)], list(FIELD_LANES))
        self.assertEqual((Lane.F3.word_len, Lane.F3.parity_len, Lane.F3.atoms), (105, 22, 5))
        self.assertEqual((Lane.D.word_len, Lane.D.parity_len, Lane.D.atoms), (30, 11, 1))
        with self.assertRaises(ValueError):
            Lane.field(4)

    def test_whitening_is_deterministic_and_balanced(self) -> None:
        for lane in Lane:
            a = lane.whitening()
            self.assertEqual(a, lane.whitening())
            self.assertEqual(len(a), lane.word_len)
            ones = sum(bin(b).count("1") for b in a)
            bits = len(a) * 8
            self.assertTrue(
                bits * 38 // 100 < ones < bits * 62 // 100, f"{lane} ones {ones}/{bits}"
            )
        self.assertNotEqual(Lane.F0.whitening(), Lane.F1.whitening())

    def test_lanes_roundtrip_through_cells(self) -> None:
        d_data = bytes(b ^ 0xA5 for b in range(D_DATA))
        d = encode_lane(Lane.D, d_data)
        fields = [encode_lane(lane, field_data(s)) for s, lane in enumerate(FIELD_LANES)]
        cells = FrameCells.from_words(d, fields)
        self.assertEqual(cells.d_word(), d)
        self.assertEqual(decode_lane(Lane.D, cells.d_word()), d_data)
        for s, (lane, word) in enumerate(zip(FIELD_LANES, fields)):
            self.assertEqual(cells.field_word(s), word)
            self.assertEqual(decode_lane(lane, cells.field_word(s)), field_data(s))
        # the spare cells at the end of every sector stay dark
        for s in range(SECTORS):
            for cell in sector_cells(s)[FIELD_WORD * 8 :]:
                self.assertFalse(cells.petals[cell])

    def test_all_zero_data_still_lights_about_half_the_petals(self) -> None:
        zero = bytes(FIELD_DATA)
        cells = FrameCells.from_words(
            encode_lane(Lane.D, bytes(D_DATA)), [encode_lane(lane, zero) for lane in FIELD_LANES]
        )
        lit = sum(cells.petals)
        self.assertTrue(1300 <= lit <= 2050, f"{lit} lit petals")

    def test_gate_dots_are_always_lit_and_guards_dark(self) -> None:
        field = b"\xff" * FIELD_WORD
        cells = FrameCells.from_words(b"\xff" * D_WORD, [field] * SECTORS)
        for slot, role in enumerate(slot_roles()):
            if role.kind in (SlotKind.GATE, SlotKind.DATA):
                self.assertTrue(cells.dots[slot])
            else:
                self.assertFalse(cells.dots[slot])

    def test_counted_decoding_reports_the_rewritten_positions(self) -> None:
        data = field_data(9)
        clean = encode_lane(Lane.F2, data)
        self.assertEqual(decode_lane_counted(Lane.F2, clean), (data, 0))
        damaged = bytearray(clean)
        for position in (0, 7, 19, 31, 60, 104):
            damaged[position] ^= 0xC3
        self.assertEqual(decode_lane_counted(Lane.F2, bytes(damaged)), (data, 6))
        # erased positions count as rewritten even when they held the right byte
        self.assertEqual(
            decode_lane_counted(Lane.F2, bytes(damaged), [0, 7, 19, 31, 60, 104, 3]), (data, 7)
        )

    def test_a_word_decoded_under_another_sector_is_refused(self) -> None:
        word = encode_lane(Lane.F0, field_data(1))
        for lane in (Lane.F1, Lane.F2, Lane.F3):
            with self.assertRaises(RsError):
                decode_lane(lane, word)

    def test_decode_survives_burst_damage_to_a_lane(self) -> None:
        data = field_data(3)
        word = bytearray(encode_lane(Lane.F1, data))
        for i in range(FIELD_PARITY // 2):
            word[i] ^= 0x5A
        self.assertEqual(decode_lane(Lane.F1, bytes(word)), data)

    def test_lane_codec_rejects_wrong_lengths(self) -> None:
        with self.assertRaises(ValueError):
            encode_lane(Lane.F0, bytes(FIELD_DATA + 1))
        with self.assertRaises(RsError) as caught:
            decode_lane(Lane.D, bytes(D_WORD - 1))
        self.assertIs(caught.exception.kind, RsErrorKind.INVALID_SHAPE)
        with self.assertRaises(ValueError):
            FrameCells.from_words(bytes(D_WORD + 1), [bytes(FIELD_WORD)] * SECTORS)
        with self.assertRaises(ValueError):
            FrameCells.from_words(bytes(D_WORD), [bytes(FIELD_WORD)] * 3)
        with self.assertRaises(ValueError):
            FrameCells([False] * TOTAL_SLOTS, [False] * (FIELD_CELLS - 1))
        with self.assertRaises(IndexError):
            FrameCells([False] * TOTAL_SLOTS, [False] * FIELD_CELLS).field_word(4)


if __name__ == "__main__":
    unittest.main()
