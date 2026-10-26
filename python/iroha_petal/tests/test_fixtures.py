# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Conformance with the shared golden vectors in ``fixtures/petal/petal_stream_v5.json``."""

from __future__ import annotations

import struct
import unittest

from petal_test_support import stream_fixture

from iroha_petal import crc32c
from iroha_petal.fountain import mask_words, mix32
from iroha_petal.glyphs import GLYPH_CHARS, GLYPH_GRID, STROKE_WIDTH, STROKES
from iroha_petal.kana import (
    KANA_BLURS,
    KANA_COARSE_GRID,
    KANA_DATA_TILES,
    KANA_GLYPH_ATOMS,
    KANA_GLYPH_PARITY,
    KANA_GLYPH_WORD,
    KANA_GRID,
    KANA_ID_BASE,
    KANA_LANES,
    KANA_POLARITY_ATOMS,
    KANA_POLARITY_PARITY,
    KANA_POLARITY_WORD,
    coarse_template_probe,
    decode_kana_lane_counted,
    encode_kana_lane,
    glyph_word,
    kana_first_id,
    kana_packet,
    polarity_word,
    template_probe,
)
from iroha_petal.lanes import (
    ATOM_LEN,
    ATOMS_PER_FRAME,
    D_PARITY,
    D_WORD,
    FIELD_ATOMS,
    FIELD_LANES,
    FIELD_PARITY,
    FIELD_WORD,
    FrameCells,
    Lane,
    decode_lane,
    encode_lane,
)
from iroha_petal.layout import (
    BACKDROP_RADIUS,
    BAND_INNER,
    BAND_SLOTS,
    BAND_SPACING,
    CANVAS,
    CARD_HEIGHT,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    KANA_BANDS,
    KANA_COLUMNS,
    KANA_CORNER,
    KANA_FINDER_CLEARANCE,
    KANA_GAP,
    KANA_GLYPH_BOX,
    KANA_MARGIN,
    KANA_PILOT_SYMBOLS,
    KANA_PILOTS,
    KANA_PITCH,
    KANA_ROWS,
    KANA_TILE,
    MASK,
    MASK_ORIGIN,
    MASK_PITCH,
    PETAL_HALF_LENGTH,
    PETAL_HALF_WIDTH,
    PETAL_NOTCH_DISTANCE,
    PETAL_NOTCH_RADIUS,
    PETAL_RADIUS,
    RING_RADII,
    RING_SLOTS,
    SECTORS,
    SQUARE_TOP,
    SlotKind,
    cell_light,
    data_slots,
    kana_tiles,
    sector_cells,
    slot_roles,
)
from iroha_petal.prng import Xorshift32
from iroha_petal.render import Palette
from iroha_petal.rs import ReedSolomon
from iroha_petal.stream import (
    BEACON_INTERVAL,
    FORMAT_VERSION,
    LaneHeader,
    StreamAssembler,
    StreamEncoder,
    first_atom_id,
    parse_atom_lane,
    parse_d_lane,
)
from iroha_petal.turns import (
    TURN_ATOMS,
    TURN_BLURS,
    TURN_CARRIERS,
    TURN_ID_BASE,
    TURN_PARITY,
    TURN_POSITIONS,
    TURN_WORD,
    carrier_order,
    decode_turn_lane_counted,
    encode_turn_lane,
    turn_first_id,
    turn_packet,
    turn_template,
    turn_whitening,
)


def flags(bitmap: bytes, count: int = FIELD_CELLS) -> list:
    """The first ``count`` flags of a bitmap (one per field cell by default), flag 0 in the
    most significant bit of byte 0."""
    return [(bitmap[i >> 3] >> (7 - (i & 7))) & 1 == 1 for i in range(count)]


def single(value: float) -> float:
    """``value`` rounded to single precision, as the reference stores its constants."""
    return struct.unpack("<f", struct.pack("<f", value))[0]


class FixtureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = stream_fixture()

    def test_constants_and_layout_match(self) -> None:
        doc = self.doc
        self.assertEqual(doc["fixture_version"], 5)
        self.assertEqual(doc["format"], "petal-stream-v5")
        constants = doc["constants"]
        self.assertEqual(constants["canvas"], CANVAS)
        self.assertEqual(constants["atom_len"], ATOM_LEN)
        self.assertEqual((constants["d_word"], constants["d_parity"]), (D_WORD, D_PARITY))
        self.assertEqual(
            (constants["field_word"], constants["field_parity"], constants["field_atoms"]),
            (FIELD_WORD, FIELD_PARITY, FIELD_ATOMS),
        )
        self.assertEqual(constants["sectors"], SECTORS)
        self.assertEqual(constants["atoms_per_frame"], ATOMS_PER_FRAME)
        self.assertEqual(constants["field_cells"], FIELD_CELLS)
        self.assertEqual(constants["beacon_interval"], BEACON_INTERVAL)
        self.assertEqual(constants["format_version"], FORMAT_VERSION)
        for key, value in (
            ("turn_positions", TURN_POSITIONS),
            ("turn_carriers", TURN_CARRIERS),
            ("turn_word", TURN_WORD),
            ("turn_parity", TURN_PARITY),
            ("turn_atoms", TURN_ATOMS),
            ("turn_id_base", TURN_ID_BASE),
            ("card_height", CARD_HEIGHT),
            ("square_top", SQUARE_TOP),
            ("kana_tile", KANA_TILE),
            ("kana_pitch", KANA_PITCH),
            ("kana_glyph_box", KANA_GLYPH_BOX),
            ("kana_corner", KANA_CORNER),
            ("kana_rows", KANA_ROWS),
            ("kana_columns", KANA_COLUMNS),
            ("kana_gap", KANA_GAP),
            ("kana_margin", KANA_MARGIN),
            ("kana_finder_clearance", KANA_FINDER_CLEARANCE),
            ("kana_bands", KANA_BANDS),
            ("kana_pilots", KANA_PILOTS),
            ("kana_data_tiles", KANA_DATA_TILES),
            ("kana_glyph_word", KANA_GLYPH_WORD),
            ("kana_glyph_parity", KANA_GLYPH_PARITY),
            ("kana_glyph_atoms", KANA_GLYPH_ATOMS),
            ("kana_polarity_word", KANA_POLARITY_WORD),
            ("kana_polarity_parity", KANA_POLARITY_PARITY),
            ("kana_polarity_atoms", KANA_POLARITY_ATOMS),
            ("kana_id_base", KANA_ID_BASE),
        ):
            self.assertEqual(constants[key], value, key)
        layout = doc["layout"]
        self.assertEqual(layout["finder_centers"], [v for c in FINDER_CENTERS for v in c])
        self.assertEqual((layout["band_inner"], layout["band_spacing"]), (BAND_INNER, BAND_SPACING))
        self.assertEqual(layout["band_slots"], list(BAND_SLOTS))
        # the petal constants are single-precision values in the reference
        for key, value in (
            ("petal_radius", PETAL_RADIUS),
            ("petal_half_length", PETAL_HALF_LENGTH),
            ("petal_half_width", PETAL_HALF_WIDTH),
            ("petal_notch_radius", PETAL_NOTCH_RADIUS),
            ("petal_notch_distance", PETAL_NOTCH_DISTANCE),
        ):
            self.assertEqual(single(layout[key]), value, key)
        sectors = layout["sector_cells"]
        self.assertEqual(len(sectors), SECTORS)
        for sector, cells in enumerate(sectors):
            self.assertEqual(cells, list(sector_cells(sector)), f"sector {sector}")
        orders = layout["carrier_orders"]
        self.assertEqual(len(orders), SECTORS)
        for sector, order in enumerate(orders):
            self.assertEqual(order, list(carrier_order(sector)), f"sector {sector}")
        self.assertEqual(tuple(layout["mask"]), MASK)
        self.assertEqual((layout["mask_origin"], layout["mask_pitch"]), (MASK_ORIGIN, MASK_PITCH))
        self.assertEqual(layout["backdrop_radius"], BACKDROP_RADIUS)
        light = flags(bytes.fromhex(layout["light_cells_hex"]))
        self.assertEqual(light, [cell_light(cell) for cell in range(FIELD_CELLS)])
        self.assertEqual(layout["ring_slots"], list(RING_SLOTS))
        self.assertEqual(layout["ring_radii"], list(RING_RADII))
        self.assertEqual(layout["dot_radius"], DOT_RADIUS)
        self.assertEqual(layout["data_slots"], list(data_slots()))
        roles = slot_roles()
        self.assertEqual(
            layout["gate_slots"], [i for i, r in enumerate(roles) if r.kind is SlotKind.GATE]
        )
        self.assertEqual(
            layout["guard_slots"], [i for i, r in enumerate(roles) if r.kind is SlotKind.GUARD]
        )
        # every katakana tile as [band, row, column, x, y, pilot] (-1 for a data tile)
        tiles = layout["kana_tiles"]
        self.assertEqual(len(tiles), len(kana_tiles()))
        for index, (tile, row) in enumerate(zip(kana_tiles(), tiles)):
            pilot = -1 if tile.pilot is None else tile.pilot
            self.assertEqual(
                [tile.band, tile.row, tile.column, tile.x, tile.y, pilot], row, f"tile {index}"
            )
        self.assertEqual(
            [tuple(symbol) for symbol in layout["kana_pilot_symbols"]],
            [(glyph, int(light)) for glyph, light in KANA_PILOT_SYMBOLS],
        )
        glyphs = doc["glyphs"]
        self.assertEqual(glyphs["chars"], GLYPH_CHARS)
        self.assertEqual((glyphs["grid"], glyphs["stroke_width"]), (GLYPH_GRID, STROKE_WIDTH))
        self.assertEqual(len(glyphs["strokes"]), len(STROKES))
        for glyph, (strokes, expected) in enumerate(zip(glyphs["strokes"], STROKES)):
            self.assertEqual(
                [[tuple(point) for point in stroke] for stroke in strokes],
                [list(stroke) for stroke in expected],
                f"glyph {glyph}",
            )

    def test_palette_matches(self) -> None:
        palette = self.doc["palette"]
        ours = Palette()
        for key in ("background", "light", "pink", "backdrop", "ink"):
            self.assertEqual(tuple(palette[key]), getattr(ours, key), key)

    def test_checksums_prng_and_whitening_match(self) -> None:
        for case in self.doc["crc32c"]:
            self.assertEqual(crc32c(bytes.fromhex(case["input_hex"])), case["crc32c"])
        prng = self.doc["prng"]
        rng = Xorshift32(1)
        self.assertEqual([rng.next_u32() for _ in range(6)], prng["xorshift32_seed1"])
        for case in prng["mix32"]:
            self.assertEqual(mix32(case["in"]), case["out"])
        whitening = self.doc["whitening"]
        turn_lanes = [f"T{sector}" for sector in range(SECTORS)]
        kana_lanes = [lane.value for lane in KANA_LANES]
        self.assertEqual(kana_lanes, ["P0", "K0", "P1", "K1"])
        self.assertEqual(
            sorted(whitening), sorted([lane.value for lane in Lane] + turn_lanes + kana_lanes)
        )
        for lane in Lane:
            self.assertEqual(lane.whitening().hex(), whitening[lane.value], lane)
        for sector, name in enumerate(turn_lanes):
            self.assertEqual(turn_whitening(sector).hex(), whitening[name], name)
        for lane in KANA_LANES:
            self.assertEqual(lane.whitening().hex(), whitening[lane.value], lane)

    def test_kana_ids_and_reader_templates_match(self) -> None:
        ids = self.doc["kana_first_ids"]
        self.assertEqual(ids["lanes"], [lane.value for lane in KANA_LANES])
        self.assertEqual(len(ids["frames"]), len(ids["ids"]))
        for frame, row in zip(ids["frames"], ids["ids"]):
            self.assertEqual([kana_first_id(frame, lane) for lane in KANA_LANES], row, frame)
        reader = self.doc["kana_reader"]
        self.assertEqual(tuple(reader["blurs"]), KANA_BLURS)
        self.assertEqual((reader["grid"], reader["coarse_grid"]), (KANA_GRID, KANA_COARSE_GRID))
        probes = reader["template_probes"]
        self.assertGreaterEqual(len(probes), len(KANA_BLURS))
        for probe in probes:
            blur = KANA_BLURS.index(probe["blur"])
            got = template_probe(blur, probe["glyph"], probe["light"] == 1, probe["i"], probe["j"])
            self.assertLess(abs(got - probe["value"]), 1e-12, f"{probe}: {got}")
        coarse = reader["coarse_probes"]
        self.assertTrue(coarse)
        for probe in coarse:
            got = coarse_template_probe(probe["glyph"], probe["light"] == 1, probe["i"], probe["j"])
            self.assertLess(abs(got - probe["value"]), 1e-12, f"{probe}: {got}")

    def test_turn_ids_and_reader_templates_match(self) -> None:
        ids = self.doc["turn_first_ids"]
        for frame, sector, atom_id in zip(ids["frames"], ids["sectors"], ids["ids"]):
            self.assertEqual(turn_first_id(frame, sector), atom_id, (frame, sector))
        reader = self.doc["turn_reader"]
        self.assertEqual(tuple(reader["blurs"]), TURN_BLURS)
        probes = reader["template_probes"]
        self.assertEqual(len(probes), 6 * len(TURN_BLURS))
        for probe in probes:
            blur = TURN_BLURS.index(probe["blur"])
            got = turn_template(blur, probe["along"], probe["across"])
            self.assertLess(
                abs(got - probe["value"]),
                1e-12,
                f"blur {probe['blur']} at ({probe['along']}, {probe['across']}): {got}",
            )

    def test_reed_solomon_vectors_encode_and_correct(self) -> None:
        for case in self.doc["reed_solomon"]:
            nsym = case["nsym"]
            data = bytes.fromhex(case["data_hex"])
            word = bytes.fromhex(case["codeword_hex"])
            rs = ReedSolomon(nsym)
            self.assertEqual(rs.encode(data), word)
            # damage up to the correction capacity and recover
            damaged = bytearray(word)
            for position in ((i * 3) % len(word) for i in range(nsym // 2)):
                damaged[position] ^= 0x5A
            rs.decode(damaged, [])
            self.assertEqual(bytes(damaged), word)
            # erase exactly nsym positions and recover
            erased = bytearray(word)
            positions = list(range(len(word) - nsym, len(word)))
            for position in positions:
                erased[position] = 0
            rs.decode(erased, positions)
            self.assertEqual(bytes(erased), word)

    def test_fountain_masks_and_atom_ids_match(self) -> None:
        for case in self.doc["fountain_masks"]:
            self.assertEqual(mask_words(case["k"], case["crc"], case["id"]), case["mask"])
        ids = self.doc["first_atom_ids"]
        for frame, atom_id in zip(ids["frames"], ids["ids"]):
            self.assertEqual(first_atom_id(frame), atom_id)

    def test_streams_encode_identically_and_reassemble(self) -> None:
        for stream in self.doc["streams"]:
            name = stream["name"]
            data = bytes.fromhex(stream["payload_hex"])
            encoder = StreamEncoder(data, stream["kind"])
            meta = encoder.meta
            self.assertEqual(meta.length, stream["len"], name)
            self.assertEqual(meta.crc, stream["crc32c"], name)
            self.assertEqual(meta.tag, stream["tag"], name)
            self.assertEqual(meta.source_atoms, stream["source_atoms"], name)
            self.assertEqual(encoder.systematic_frames(), stream["systematic_frames"], name)
            for frame in stream["frames"]:
                number = frame["frame"]
                label = f"{name} frame {number}"
                lanes = encoder.lane_data(number)
                self.assertEqual(lanes.d.hex(), frame["d_data"], label)
                self.assertEqual([f.hex() for f in lanes.fields], frame["field_data"], label)
                turn_data = [bytes.fromhex(data) for data in frame["turn_data"]]
                self.assertEqual(list(lanes.turns), turn_data, label)
                kana_data = [bytes.fromhex(data) for data in frame["kana_data"]]
                kana_words = [bytes.fromhex(word) for word in frame["kana_words"]]
                self.assertEqual(list(lanes.kana), kana_data, label)
                self.assertEqual(list(encoder.words(number).kana), kana_words, label)
                kana_header = LaneHeader(meta.tag, number)
                for lane, data_bytes, word in zip(KANA_LANES, kana_data, kana_words):
                    self.assertEqual(encode_kana_lane(lane, data_bytes), word, f"{label} {lane}")
                    packet = kana_packet(kana_header, lane, data_bytes)
                    self.assertIsNotNone(packet, f"{label} {lane} check")
                    self.assertEqual(packet.first_id, kana_first_id(number, lane))
                    self.assertEqual(len(packet.atoms), lane.atoms)
                d_word = bytes.fromhex(frame["d_word"])
                field_words = [bytes.fromhex(word) for word in frame["field_words"]]
                turn_words = [bytes.fromhex(word) for word in frame["turn_words"]]
                header = LaneHeader(meta.tag, number)
                for sector, word in enumerate(turn_words):
                    self.assertEqual(
                        encode_turn_lane(sector, turn_data[sector]), word, f"{label} T{sector}"
                    )
                    packet = turn_packet(header, sector, turn_data[sector])
                    self.assertIsNotNone(packet, f"{label} turn check {sector}")
                    self.assertEqual(packet.first_id, turn_first_id(number, sector))
                self.assertEqual(encode_lane(Lane.D, lanes.d), d_word, label)
                for lane, word in zip(FIELD_LANES, field_words):
                    self.assertEqual(encode_lane(lane, lanes.lane(lane)), word, f"{label} {lane}")
                    self.assertEqual(decode_lane(lane, word), lanes.lane(lane), f"{label} {lane}")
                cells = (
                    FrameCells.from_words(d_word, field_words)
                    .with_turns(turn_words)
                    .with_kana(kana_words)
                )
                turns = "".join(str(turn) for turn in cells.turns)
                self.assertEqual(turns, frame["turns"], label)
                glyphs = "".join("%x" % symbol.glyph for symbol in cells.kana)
                self.assertEqual(glyphs, frame["kana_glyphs"], label)
                self.assertEqual(
                    [symbol.light for symbol in cells.kana],
                    flags(bytes.fromhex(frame["kana_light_hex"]), len(cells.kana)),
                    label,
                )
                for band in range(KANA_BANDS):
                    self.assertEqual(polarity_word(cells.kana, band), kana_words[2 * band])
                    self.assertEqual(glyph_word(cells.kana, band), kana_words[2 * band + 1])
                lit = [i for i, on in enumerate(cells.dots) if on]
                self.assertEqual(lit, frame["lit_dots"], label)
                self.assertEqual(
                    list(cells.petals), flags(bytes.fromhex(frame["lit_petals_hex"])), label
                )
                self.assertEqual(cells, encoder.cells(number), label)
            # push every fixture frame through decode_lane + assembler, without the turn and
            # kana lanes, with the turn lanes, and with both: each layer finishes the one-pass
            # stream in fewer frames
            if name == "one-pass":
                needed = []
                for layers in (0, 1, 2):
                    with_turns = layers >= 1
                    with_kana = layers >= 2
                    assembler = StreamAssembler()
                    count = 0
                    for frame in stream["frames"]:
                        count += 1
                        d = decode_lane(Lane.D, bytes.fromhex(frame["d_word"]))
                        assembler.push_d_lane(parse_d_lane(d))
                        for lane, word in zip(FIELD_LANES, frame["field_words"]):
                            data_bytes = decode_lane(lane, bytes.fromhex(word))
                            assembler.push_atoms(parse_atom_lane(lane, data_bytes))
                        if with_turns:
                            header = LaneHeader(meta.tag, frame["frame"])
                            for sector, word in enumerate(frame["turn_words"]):
                                data_bytes, _ = decode_turn_lane_counted(
                                    sector, bytes.fromhex(word)
                                )
                                assembler.push_atoms(turn_packet(header, sector, data_bytes))
                        if with_kana:
                            header = LaneHeader(meta.tag, frame["frame"])
                            for lane, word in zip(KANA_LANES, frame["kana_words"]):
                                data_bytes, _ = decode_kana_lane_counted(lane, bytes.fromhex(word))
                                assembler.push_atoms(kana_packet(header, lane, data_bytes))
                        if assembler.progress().complete:
                            break
                    completed = assembler.take_completed()
                    self.assertIsNotNone(completed)
                    self.assertEqual(completed.payload, data)
                    self.assertEqual(completed.meta.kind, stream["kind"])
                    needed.append(count)
                self.assertTrue(needed[2] < needed[1] < needed[0], needed)


if __name__ == "__main__":
    unittest.main()
