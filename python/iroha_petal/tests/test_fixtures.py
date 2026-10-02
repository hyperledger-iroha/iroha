# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Conformance with the shared golden vectors in ``fixtures/petal/petal_stream_v1.json``."""

from __future__ import annotations

import unittest

from petal_test_support import stream_fixture

from iroha_petal import crc32c
from iroha_petal.fountain import mask_words, mix32
from iroha_petal.glyphs import GLYPH_CHARS, STROKE_WIDTH, STROKES, TEMPLATES
from iroha_petal.lanes import (
    ATOM_LEN,
    D_PARITY,
    D_WORD,
    K_PARITY,
    K_WORD,
    P_PARITY,
    P_WORD,
    FrameCells,
    Lane,
    decode_lane,
    encode_lane,
)
from iroha_petal.layout import (
    CANVAS,
    DOT_RADIUS,
    FINDER_CENTERS,
    MASK,
    RING_RADII,
    RING_SLOTS,
    TILE_ORIGIN,
    TILE_PITCH,
    TILE_SIZE,
    TILES,
    SlotKind,
    data_slots,
    slot_roles,
)
from iroha_petal.prng import Xorshift32
from iroha_petal.rs import ReedSolomon
from iroha_petal.stream import (
    BEACON_INTERVAL,
    FORMAT_VERSION,
    StreamAssembler,
    StreamEncoder,
    first_atom_id,
    parse_atom_lane,
    parse_d_lane,
)


class FixtureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = stream_fixture()

    def test_constants_and_layout_match(self) -> None:
        doc = self.doc
        self.assertEqual(doc["fixture_version"], 1)
        self.assertEqual(doc["format"], "petal-stream-v1")
        constants = doc["constants"]
        self.assertEqual(constants["canvas"], CANVAS)
        self.assertEqual(constants["tile_origin"], TILE_ORIGIN)
        self.assertEqual(constants["tile_pitch"], TILE_PITCH)
        self.assertEqual(constants["tile_size"], TILE_SIZE)
        self.assertEqual(constants["dot_radius"], DOT_RADIUS)
        self.assertEqual(constants["atom_len"], ATOM_LEN)
        self.assertEqual(
            (constants["p_word"], constants["k_word"], constants["d_word"]),
            (P_WORD, K_WORD, D_WORD),
        )
        self.assertEqual(
            (constants["p_parity"], constants["k_parity"], constants["d_parity"]),
            (P_PARITY, K_PARITY, D_PARITY),
        )
        self.assertEqual(constants["beacon_interval"], BEACON_INTERVAL)
        self.assertEqual(constants["format_version"], FORMAT_VERSION)
        layout = doc["layout"]
        self.assertEqual(tuple(layout["mask"]), MASK)
        self.assertEqual(layout["tiles_col_row"], [v for tile in TILES for v in tile])
        self.assertEqual(layout["ring_slots"], list(RING_SLOTS))
        self.assertEqual(layout["ring_radii"], list(RING_RADII))
        self.assertEqual(layout["finder_centers"], [v for c in FINDER_CENTERS for v in c])
        self.assertEqual(layout["data_slots"], list(data_slots()))
        roles = slot_roles()
        self.assertEqual(
            layout["gate_slots"], [i for i, r in enumerate(roles) if r.kind is SlotKind.GATE]
        )
        self.assertEqual(
            layout["guard_slots"], [i for i, r in enumerate(roles) if r.kind is SlotKind.GUARD]
        )

    def test_glyph_alphabet_strokes_and_templates_match(self) -> None:
        glyphs = self.doc["glyphs"]
        self.assertEqual(glyphs["chars"], GLYPH_CHARS)
        self.assertAlmostEqual(glyphs["stroke_width"], STROKE_WIDTH, delta=1e-9)
        self.assertEqual(len(glyphs["strokes"]), len(STROKES))
        for glyph, polylines in enumerate(glyphs["strokes"]):
            ours = [[v for point in stroke for v in point] for stroke in STROKES[glyph]]
            self.assertEqual(polylines, ours, f"glyph {glyph}")
        self.assertEqual(len(glyphs["templates"]), len(TEMPLATES))
        for glyph, row in enumerate(glyphs["templates"]):
            self.assertEqual(tuple(row), TEMPLATES[glyph], f"glyph {glyph}")

    def test_checksums_prng_and_whitening_match(self) -> None:
        for case in self.doc["crc32c"]:
            self.assertEqual(crc32c(bytes.fromhex(case["input_hex"])), case["crc32c"])
        prng = self.doc["prng"]
        rng = Xorshift32(1)
        self.assertEqual([rng.next_u32() for _ in range(6)], prng["xorshift32_seed1"])
        for case in prng["mix32"]:
            self.assertEqual(mix32(case["in"]), case["out"])
        whitening = self.doc["whitening"]
        for lane in Lane:
            self.assertEqual(lane.whitening().hex(), whitening[lane.value])

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
                p, k, d = encoder.lane_data(number)
                self.assertEqual(p.hex(), frame["p_data"], label)
                self.assertEqual(k.hex(), frame["k_data"], label)
                self.assertEqual(d.hex(), frame["d_data"], label)
                pw = bytes.fromhex(frame["p_word"])
                kw = bytes.fromhex(frame["k_word"])
                dw = bytes.fromhex(frame["d_word"])
                self.assertEqual(encode_lane(Lane.P, p), pw, label)
                self.assertEqual(encode_lane(Lane.K, k), kw, label)
                self.assertEqual(encode_lane(Lane.D, d), dw, label)
                self.assertEqual(decode_lane(Lane.K, kw), k, label)
                cells = FrameCells.from_words(pw, kw, dw)
                self.assertEqual("".join("%x" % g for g in cells.glyph), frame["glyphs"], label)
                lit = [i for i, on in enumerate(cells.dots) if on]
                self.assertEqual(lit, frame["lit_dots"], label)
                self.assertEqual(cells, encoder.cells(number), label)
            # push every fixture frame through decode_lane + assembler
            if name == "one-pass":
                assembler = StreamAssembler()
                for frame in stream["frames"]:
                    d = decode_lane(Lane.D, bytes.fromhex(frame["d_word"]))
                    assembler.push_d_lane(parse_d_lane(d))
                    for lane, key in ((Lane.P, "p_word"), (Lane.K, "k_word")):
                        data_bytes = decode_lane(lane, bytes.fromhex(frame[key]))
                        assembler.push_atoms(parse_atom_lane(lane, data_bytes))
                completed = assembler.take_completed()
                self.assertIsNotNone(completed)
                self.assertEqual(completed.payload, data)
                self.assertEqual(completed.meta.kind, stream["kind"])


if __name__ == "__main__":
    unittest.main()
