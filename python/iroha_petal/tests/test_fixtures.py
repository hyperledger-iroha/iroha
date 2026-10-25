# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Conformance with the shared golden vectors in ``fixtures/petal/petal_stream_v2.json``."""

from __future__ import annotations

import struct
import unittest

from petal_test_support import stream_fixture

from iroha_petal import crc32c
from iroha_petal.fountain import mask_words, mix32
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
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
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
    SlotKind,
    cell_light,
    data_slots,
    sector_cells,
    slot_roles,
)
from iroha_petal.prng import Xorshift32
from iroha_petal.render import Palette
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


def flags(bitmap: bytes) -> list:
    """One flag per field cell, cell 0 in the most significant bit of byte 0."""
    return [(bitmap[cell >> 3] >> (7 - (cell & 7))) & 1 == 1 for cell in range(FIELD_CELLS)]


def single(value: float) -> float:
    """``value`` rounded to single precision, as the reference stores its constants."""
    return struct.unpack("<f", struct.pack("<f", value))[0]


class FixtureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = stream_fixture()

    def test_constants_and_layout_match(self) -> None:
        doc = self.doc
        self.assertEqual(doc["fixture_version"], 2)
        self.assertEqual(doc["format"], "petal-stream-v2")
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

    def test_palette_matches(self) -> None:
        palette = self.doc["palette"]
        ours = Palette()
        for key in ("background", "light", "pink", "backdrop"):
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
        self.assertEqual(sorted(whitening), sorted(lane.value for lane in Lane))
        for lane in Lane:
            self.assertEqual(lane.whitening().hex(), whitening[lane.value], lane)

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
                d_word = bytes.fromhex(frame["d_word"])
                field_words = [bytes.fromhex(word) for word in frame["field_words"]]
                self.assertEqual(encode_lane(Lane.D, lanes.d), d_word, label)
                for lane, word in zip(FIELD_LANES, field_words):
                    self.assertEqual(encode_lane(lane, lanes.lane(lane)), word, f"{label} {lane}")
                    self.assertEqual(decode_lane(lane, word), lanes.lane(lane), f"{label} {lane}")
                cells = FrameCells.from_words(d_word, field_words)
                lit = [i for i, on in enumerate(cells.dots) if on]
                self.assertEqual(lit, frame["lit_dots"], label)
                self.assertEqual(
                    list(cells.petals), flags(bytes.fromhex(frame["lit_petals_hex"])), label
                )
                self.assertEqual(cells, encoder.cells(number), label)
            # push every fixture frame through decode_lane + assembler
            if name == "one-pass":
                assembler = StreamAssembler()
                for frame in stream["frames"]:
                    d = decode_lane(Lane.D, bytes.fromhex(frame["d_word"]))
                    assembler.push_d_lane(parse_d_lane(d))
                    for lane, word in zip(FIELD_LANES, frame["field_words"]):
                        data_bytes = decode_lane(lane, bytes.fromhex(word))
                        assembler.push_atoms(parse_atom_lane(lane, data_bytes))
                completed = assembler.take_completed()
                self.assertIsNotNone(completed)
                self.assertEqual(completed.payload, data)
                self.assertEqual(completed.meta.kind, stream["kind"])


if __name__ == "__main__":
    unittest.main()
