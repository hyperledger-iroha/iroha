# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Turn lanes: carrier order, the turn lane codec and the turn reader (ports of the
``turns.rs`` unit tests)."""

from __future__ import annotations

import math
import struct
import unittest
from dataclasses import replace

from petal_test_support import camera_homography, capture, payload

from iroha_petal import (
    DecodedFrame,
    FrameCells,
    LaneHeader,
    Luma,
    RenderOptions,
    RsError,
    StreamEncoder,
    Xorshift32,
    decode_frame,
    render_frame,
)
from iroha_petal.lanes import D_WORD, FIELD_DATA, FIELD_LANES, FIELD_WORD, encode_lane
from iroha_petal.layout import (
    BAND_COUNT,
    BAND_OFFSETS,
    FIELD_CELLS,
    PETAL_HALF_LENGTH,
    PETAL_RADIUS,
    SECTORS,
    cell_angle,
    cell_band_slot,
    cell_center,
    sector_cells,
)
from iroha_petal.stream import first_atom_id
from iroha_petal.turns import (
    TURN_ATOMS,
    TURN_BLURS,
    TURN_CARRIERS,
    TURN_DATA,
    TURN_ID_BASE,
    TURN_POSITIONS,
    TURN_WORD,
    _decide,
    _neighbours,
    _sample_window,
    _tables,
    carrier_order,
    carrier_position,
    carriers,
    decode_turn_lane_counted,
    encode_turn_lane,
    observed_turns,
    read_turns,
    turn_angle,
    turn_check,
    turn_data,
    turn_first_id,
    turn_label,
    turn_packet,
    turn_position,
    turn_template,
)


def atoms(seed: int) -> bytes:
    """The reference tests' turn atoms: byte ``b`` is ``b * 29 ^ seed``."""
    return bytes(((b * 29) & 0xFF) ^ seed for b in range(TURN_ATOMS * 16))


#: The header the reference tests turn their cells under.
HEADER = LaneHeader(1, 2)


def turn_words(header: LaneHeader = HEADER) -> list:
    """The transmitted turn codewords of ``atoms(sector)`` under ``header``."""
    return [encode_turn_lane(s, turn_data(header, s, atoms(s))) for s in range(SECTORS)]


def stream() -> StreamEncoder:
    """The turn reader tests' stream: 6,000 bytes from xorshift32 seed 5, kind 2."""
    return StreamEncoder(payload(6_000, 5), 2)


def rendered(cells: FrameCells) -> Luma:
    """A 768-pixel render, 2 x 2 supersampled, as the reference tests draw one."""
    return render_frame(cells, RenderOptions(size=768, supersample=2)).to_luma()


class TurnLaneTest(unittest.TestCase):
    def test_lane_sizes_are_consistent(self) -> None:
        self.assertEqual((TURN_WORD, TURN_DATA), (90, 68))
        self.assertEqual(turn_first_id(0, 0), 1 << 31)
        self.assertEqual(turn_first_id(1, 3), (1 << 31) + 16 + 12)
        self.assertEqual(turn_first_id(0xFFFF, 3) + 3, (1 << 31) + 16 * 65_535 + 15)
        # the base schedule stays far below the turn ids however long a stream runs
        self.assertLess(first_atom_id(0xFFFF) + 21, TURN_ID_BASE)
        with self.assertRaises(ValueError):
            turn_first_id(65_536, 0)
        with self.assertRaises(IndexError):
            turn_first_id(0, 4)

    def test_gray_labels_differ_in_one_bit_between_neighbouring_turns(self) -> None:
        for position in range(TURN_POSITIONS):
            self.assertEqual(turn_position(turn_label(position)), position)
            following = (position + 1) % TURN_POSITIONS
            self.assertEqual(bin(turn_label(position) ^ turn_label(following)).count("1"), 1)
        self.assertLess(abs(turn_angle(2) - math.pi / 2.0), 1e-12)
        self.assertEqual(
            [turn_angle(p) for p in range(4)], [0.0, math.pi / 4, math.pi / 2, 3 * math.pi / 4]
        )
        for bad in (-1, 4):
            with self.assertRaises(ValueError):
                turn_angle(bad)
            with self.assertRaises(ValueError):
                turn_label(bad)
            with self.assertRaises(ValueError):
                turn_position(bad)

    def test_carrier_orders_shuffle_each_sectors_codeword_cells(self) -> None:
        for sector in range(SECTORS):
            order = list(carrier_order(sector))
            cells = list(sector_cells(sector)[: FIELD_WORD * 8])
            self.assertNotEqual(order, cells)
            self.assertEqual(sorted(order), sorted(cells))

        # the four shuffles differ
        def places(sector: int) -> list:
            cells = sector_cells(sector)
            return [cells.index(cell) for cell in carrier_order(sector)]

        self.assertNotEqual(places(0), places(1))
        with self.assertRaises(IndexError):
            carrier_order(4)

    def test_a_sector_with_too_few_lit_petals_carries_nothing(self) -> None:
        petals = [False] * FIELD_CELLS
        order = carrier_order(2)
        for cell in order[: TURN_CARRIERS - 1]:
            petals[cell] = True
        self.assertIsNone(carriers(petals, 2))
        petals[order[TURN_CARRIERS + 7]] = True
        chosen = carriers(petals, 2)
        self.assertEqual(len(chosen), TURN_CARRIERS)
        self.assertEqual(chosen[TURN_CARRIERS - 1], order[TURN_CARRIERS + 7])
        with self.assertRaises(IndexError):
            carriers(petals, 4)
        with self.assertRaises(ValueError):
            carriers(petals[:10], 0)

    def test_turn_lanes_roundtrip_and_bind_their_header(self) -> None:
        header = LaneHeader(7, 300)
        data = turn_data(header, 1, atoms(3))
        word = encode_turn_lane(1, data)
        self.assertEqual(len(word), TURN_WORD)
        decoded, corrected = decode_turn_lane_counted(1, word)
        self.assertEqual((decoded, corrected), (data, 0))
        packet = turn_packet(header, 1, decoded)
        self.assertIsNotNone(packet, "check matches")
        self.assertEqual(packet.first_id, turn_first_id(300, 1))
        self.assertEqual(len(packet.atoms), TURN_ATOMS)
        self.assertEqual(packet.atoms[2], atoms(3)[32:48])
        self.assertEqual(packet.header, header)
        # the check closes the data, big-endian
        self.assertEqual(int.from_bytes(data[64:], "big"), turn_check(header, 1, atoms(3)))
        # another frame, tag or sector does not match the check
        self.assertIsNone(turn_packet(LaneHeader(7, 301), 1, decoded))
        self.assertIsNone(turn_packet(LaneHeader(8, 300), 1, decoded))
        self.assertIsNone(turn_packet(header, 2, decoded))
        self.assertIsNone(turn_packet(header, 4, decoded))
        self.assertIsNone(turn_packet(header, 1, decoded[:67]))
        # a codeword whitened for another sector does not decode
        with self.assertRaises(RsError):
            decode_turn_lane_counted(0, word)
        # eleven wrong bytes are corrected and counted
        damaged = bytearray(word)
        for position in (0, 9, 18, 27, 36, 45, 54, 63, 72, 81, 89):
            damaged[position] ^= 0x5A
        self.assertEqual(decode_turn_lane_counted(1, bytes(damaged)), (data, 11))
        with self.assertRaises(RsError):
            decode_turn_lane_counted(1, word[:89])
        with self.assertRaises(ValueError):
            turn_data(header, 1, bytes(63))
        with self.assertRaises(ValueError):
            encode_turn_lane(1, bytes(67))

    def test_carriers_show_the_codeword_two_bits_at_a_time(self) -> None:
        fields = [
            encode_lane(lane, bytes(((b * 13) & 0xFF) ^ s for b in range(FIELD_DATA)))
            for s, lane in enumerate(FIELD_LANES)
        ]
        cells = FrameCells.from_words(bytes(D_WORD), fields)
        self.assertEqual(set(cells.turns), {0})
        words = turn_words()
        turned_cells = cells.with_turns(words)
        self.assertEqual(set(cells.turns), {0}, "the original cells stay unturned")
        carried = 0
        for sector, word in enumerate(words):
            self.assertEqual(turned_cells.turn_word(sector), word)
            chosen = carriers(turned_cells.petals, sector)
            for j, cell in enumerate(chosen):
                self.assertEqual(turned_cells.turns[cell], carrier_position(word, j))
            carried += sum(1 for cell in chosen if turned_cells.turns[cell] != 0)
        # dark petals and lit petals beyond the carriers stay unturned
        turned = sum(1 for turn in turned_cells.turns if turn != 0)
        self.assertEqual(turned, carried)
        self.assertGreater(turned, SECTORS * TURN_CARRIERS * 6 // 10)
        # the turns are part of the cells
        self.assertNotEqual(turned_cells, cells)
        self.assertEqual(FrameCells(cells.dots, cells.petals, turned_cells.turns), turned_cells)
        # turning again replaces every turn of the carriers
        again = turn_words(LaneHeader(9, 4))
        for sector, word in enumerate(again):
            self.assertEqual(turned_cells.with_turns(again).turn_word(sector), word)
        # a sector with too few lit petals has no turn word and stays unturned
        dark = FrameCells([False] * len(cells.dots), [False] * FIELD_CELLS).with_turns(words)
        self.assertEqual(set(dark.turns), {0})
        self.assertIsNone(dark.turn_word(0))
        with self.assertRaises(ValueError):
            cells.with_turns(words[:3])
        with self.assertRaises(ValueError):
            cells.with_turns(words[:3] + [bytes(89)])
        with self.assertRaises(IndexError):
            cells.turn_word(4)
        with self.assertRaises(ValueError):
            FrameCells(cells.dots, cells.petals, (4,) * FIELD_CELLS)
        with self.assertRaises(ValueError):
            FrameCells(cells.dots, cells.petals, (0,) * (FIELD_CELLS - 1))
        with self.assertRaises(IndexError):
            carrier_position(words[0], 360)


class TurnReaderTest(unittest.TestCase):
    def test_a_turned_petal_stays_out_of_its_neighbours_sampling_discs(self) -> None:
        # the nearest petal centres (radially adjacent cells) are 11 units apart; a petal
        # reaches its half-length from its centre whichever way it is turned
        nearest = min(
            math.hypot(along, across) for near in _neighbours() for _, along, across, _ in near
        )
        self.assertLess(abs(nearest - 11.0), 1e-9, nearest)
        clearance = struct.unpack("<f", struct.pack("<f", PETAL_HALF_LENGTH + PETAL_RADIUS))[0]
        self.assertGreater(nearest, clearance, nearest)

    def test_neighbours_are_the_reference_scan_of_two_bands_either_side(self) -> None:
        # the table scans one band either side: cells two bands away are 22 units apart or
        # more. It must equal the reference's scan of two bands either side, entry for entry.
        table = _neighbours()
        for cell in range(FIELD_CELLS):
            x, y = cell_center(cell)
            angle = cell_angle(cell)
            sin, cos = math.sin(angle), math.cos(angle)
            band, _ = cell_band_slot(cell)
            expected = []
            for other in range(
                BAND_OFFSETS[max(band - 2, 0)], BAND_OFFSETS[min(band + 3, BAND_COUNT)]
            ):
                if other == cell:
                    continue
                ox, oy = cell_center(other)
                dx, dy = ox - x, oy - y
                if dx * dx + dy * dy <= 19.0 * 19.0:
                    expected.append(
                        (
                            other,
                            dx * cos + dy * sin,
                            -dx * sin + dy * cos,
                            cell_angle(other) - angle,
                        )
                    )
            self.assertEqual(list(table[cell]), expected, f"cell {cell}")

    def test_windows_sample_the_image_as_the_reference_does(self) -> None:
        # a window inside the image takes a path without the border cases; every window,
        # inside, across the border or outside the image, samples what Luma.sample samples
        # at (o + b v) + a u
        encoder = stream()
        image = rendered(encoder.cells(2))
        pose = decode_frame(image).pose
        rng = Xorshift32(17)
        for trial in range(400):
            cell = rng.next_u32() % FIELD_CELLS
            if trial % 4 == 0:
                shift = ((rng.next_u32() % 2001) - 1000.0, (rng.next_u32() % 2001) - 1000.0)
            else:
                shift = ((rng.next_u32() % 7) - 3.0, (rng.next_u32() % 7) - 3.0)
            got, nan = _sample_window(image, pose, cell, *shift)
            x, y = cell_center(cell)
            x, y = x + shift[0], y + shift[1]
            sin, cos = math.sin(cell_angle(cell)), math.cos(cell_angle(cell))
            ox, oy = pose.apply(x, y)
            ax, ay = pose.apply(x + cos, y + sin)
            cx, cy = pose.apply(x - sin, y + cos)
            expected = []
            for b in range(-6, 7):
                row_x, row_y = ox + b * (cx - ox), oy + b * (cy - oy)
                for a in range(-6, 7):
                    expected.append(image.sample(row_x + a * (ax - ox), row_y + a * (ay - oy)))
            self.assertEqual(got, expected, f"cell {cell}, shift {shift}")
            self.assertFalse(nan)

    def test_templates_hold_a_unit_petal_and_kernels_tell_the_turns_apart(self) -> None:
        self.assertGreater(turn_template(0, 0.0, 0.0), 0.99)
        self.assertLess(turn_template(0, 0.0, 6.0), 0.05)
        self.assertLess(turn_template(0, 8.5, 0.0), 0.01)
        self.assertGreater(turn_template(0, 4.5, 0.0), turn_template(0, 0.0, 3.4))
        # the notch: the tip is darker than the same point at the other end of the petal
        self.assertLess(turn_template(0, 5.5, 0.0), turn_template(0, -5.5, 0.0))
        # zero outside the table
        self.assertEqual(turn_template(0, 9.0, 0.0), 0.0)
        self.assertEqual(turn_template(5, 0.0, -9.01), 0.0)
        for kernels in _tables().kernels:
            for k, kernel in enumerate(kernels):
                # a window showing exactly the petal at turn k picks turn k
                position, margin = _decide(kernel, kernels)
                self.assertEqual(position, k)
                self.assertGreater(margin, 0.0)
        with self.assertRaises(IndexError):
            turn_template(len(TURN_BLURS), 0.0, 0.0)

    def test_a_clean_render_reads_every_turn_lane_and_skips_what_it_is_told(self) -> None:
        encoder = stream()
        truth = encoder.lane_data(6)
        image = rendered(encoder.cells(6))
        decoded = decode_frame(image)
        self.assertEqual(decoded.lanes, "D0123")
        turns = read_turns(image, decoded)
        for sector, result in enumerate(turns):
            self.assertIsNotNone(result, f"turn lane {sector}")
            self.assertEqual(result.data, truth.turns[sector])
            self.assertEqual((result.corrected, result.erasures), (0, 0))
        turns = read_turns(image, decoded, (True, False, True, True))
        self.assertEqual([result is not None for result in turns], [False, True, False, False])
        self.assertEqual(read_turns(image, decoded, (True,) * SECTORS), (None,) * SECTORS)
        # a sector whose field lane did not decode has no turn lane
        partial = replace(decoded, fields=decoded.fields[:2] + (None,) + decoded.fields[3:])
        turns = read_turns(image, partial)
        self.assertTrue(turns[2] is None and turns[3] is not None)
        # the diagnostics see every carrier of the four sectors, each at its drawn turn
        observed = observed_turns(image, decoded)
        self.assertEqual(len(observed.carriers), SECTORS * TURN_CARRIERS)
        self.assertIn(observed.blur, TURN_BLURS)
        drawn = encoder.cells(6)
        for cell, position, lead in observed.carriers:
            self.assertEqual(drawn.turns[cell], position, f"cell {cell}")
            self.assertGreater(lead, 0.0)
        none = DecodedFrame(decoded.pose, decoded.rotation, decoded.mirrored, decoded.d)
        self.assertIsNone(observed_turns(image, none))
        self.assertEqual(read_turns(image, none), (None,) * SECTORS)
        # malformed arguments; a plane that does not match its size reads nothing
        with self.assertRaises(ValueError):
            read_turns(image, decoded, (False,))
        broken = Luma(10, 10)
        broken.data = bytes(5)
        self.assertEqual(read_turns(broken, decoded), (None,) * SECTORS)
        self.assertIsNone(observed_turns(Luma(0, 0), decoded))

    def test_a_camera_reads_the_turn_lanes(self) -> None:
        # the test camera at 720p, rotated and tilted, sensor noise on (the reference uses its
        # simulator's modern camera)
        encoder = stream()
        truth = encoder.lane_data(9)
        source = rendered(encoder.cells(9))
        for seed, rotation in ((1, 17.0), (2, 205.0)):
            forward = camera_homography(1280, 720, 0.8, rotation, tilt=0.15)
            image = capture(source, forward, 1280, 720, noise_seed=seed)
            decoded = decode_frame(image)
            turns = read_turns(image, decoded)
            for sector, (turn, field) in enumerate(zip(turns, decoded.fields)):
                if field is not None:
                    self.assertIsNotNone(turn, f"seed {seed} sector {sector}")
                    self.assertEqual(turn.data, truth.turns[sector], f"seed {seed} sector {sector}")
            self.assertTrue(any(turn is not None for turn in turns), f"seed {seed}")

    def test_unturned_petals_never_pass_for_a_turn_lane(self) -> None:
        cells = replace(stream().cells(3), turns=())
        image = rendered(cells)
        decoded = decode_frame(image)
        self.assertEqual(decoded.lanes, "D0123", "the field lanes read without turns")
        self.assertEqual(read_turns(image, decoded), (None,) * SECTORS)


if __name__ == "__main__":
    unittest.main()
