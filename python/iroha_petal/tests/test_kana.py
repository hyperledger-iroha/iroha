# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The card: katakana glyphs, the band layout, the kana lane codec, the band reader and the
session's band tries (ports of the ``glyphs.rs``, ``kana.rs`` and card ``layout.rs`` unit
tests, the reference's katakana render test and its upright-camera session test)."""

from __future__ import annotations

import math
import unittest
from dataclasses import replace
from functools import reduce
from operator import add

from petal_test_support import camera_homography, capture, payload, rendered

from iroha_petal import (
    DecodedFrame,
    FrameCells,
    LaneHeader,
    Luma,
    RenderOptions,
    RsError,
    ScanSession,
    StreamEncoder,
    Xorshift32,
    decode_frame,
    render_frame,
    render_square,
)
from iroha_petal.glyphs import (
    GLYPH_CHARS,
    GLYPH_COUNT,
    GLYPH_GRID,
    STROKE_WIDTH,
    STROKES,
    inked_flags,
    is_inked,
)
from iroha_petal.kana import (
    KANA_BLURS,
    KANA_COARSE_GRID,
    KANA_DATA_TILES,
    KANA_GLYPH_WORD,
    KANA_GRID,
    KANA_ID_BASE,
    KANA_LANES,
    KANA_POLARITY_WORD,
    KANA_TILE_COUNT,
    KanaKind,
    KanaLane,
    KanaSymbol,
    coarse_template_probe,
    decode_kana_lane_counted,
    encode_kana_lane,
    glyph_word,
    kana_check,
    kana_data,
    kana_first_id,
    kana_packet,
    observe_band,
    pilot_symbols,
    polarity_word,
    read_kana,
    set_kana_symbols,
    template_probe,
)
from iroha_petal.layout import (
    CANVAS,
    CARD_HEIGHT,
    KANA_BAND_HEIGHT,
    KANA_BANDS,
    KANA_GLYPH_BOX,
    KANA_PILOT_SYMBOLS,
    KANA_PILOTS,
    KANA_ROWS,
    KANA_TILE,
    SQUARE_TOP,
    kana_data_tiles,
    kana_inked,
    kana_inside,
    kana_pilot_tiles,
    kana_tile_at,
    kana_tiles,
)
from iroha_petal.render import Palette, card_height_px, draw_list, kana_glyph_strokes
from iroha_petal.session import KANA_BACKOFF, KANA_MEMORY
from iroha_petal.turns import turn_first_id


def coverage(glyph: int) -> list:
    """Inked share of every cell of an 8 x 8 grid over the design square."""
    cell = GLYPH_GRID / 8.0
    values = []
    for i in range(64):
        u, v = float(i % 8), float(i // 8)
        inked = 0
        for sy in range(8):
            for sx in range(8):
                x = (u + (sx + 0.5) / 8.0) * cell
                y = (v + (sy + 0.5) / 8.0) * cell
                inked += is_inked(glyph, x, y)
        values.append(inked / 64.0)
    return values


class GlyphTest(unittest.TestCase):
    def test_every_glyph_has_ink_inside_the_design_grid(self) -> None:
        self.assertEqual(len(GLYPH_CHARS), GLYPH_COUNT)
        self.assertEqual((GLYPH_CHARS[0], GLYPH_CHARS[15]), ("イ", "ン"))
        for glyph, strokes in enumerate(STROKES):
            self.assertGreater(reduce(add, coverage(glyph)), 6.0, f"glyph {glyph}")
            for stroke in strokes:
                for x, y in stroke:
                    self.assertTrue(2.0 <= x <= 30.0 and 2.0 <= y <= 30.0)
        with self.assertRaises(IndexError):
            is_inked(16, 0.0, 0.0)

    def test_glyphs_are_pairwise_distinct_under_blur(self) -> None:
        # zero-mean cosine distance of the coarse coverage must stay well apart
        def feature(glyph: int) -> list:
            values = coverage(glyph)
            mean = reduce(add, values) / len(values)
            centred = [v - mean for v in values]
            norm = math.sqrt(reduce(add, (v * v for v in centred)))
            return [v / norm for v in centred]

        features = [feature(glyph) for glyph in range(GLYPH_COUNT)]
        for a in range(GLYPH_COUNT):
            for b in range(a + 1, GLYPH_COUNT):
                dot = reduce(add, (x * y for x, y in zip(features[a], features[b])))
                self.assertGreater(1.0 - dot, 0.2, f"glyphs {a} and {b}: {1.0 - dot}")

    def test_a_row_of_points_is_classified_as_point_by_point(self) -> None:
        # the spans narrow and widen the stroke a hair; every point between is decided by
        # the reference's own test, so the flags never differ from it
        rng = Xorshift32(91)
        for trial in range(160):
            glyph = trial % GLYPH_COUNT
            y = (rng.next_u32() % 40_000) / 1_000.0 - 4.0
            step = 0.01 + (rng.next_u32() % 1_000) / 1_000.0
            start = (rng.next_u32() % 4_000) / 1_000.0 - 6.0
            xs = [start + step * i for i in range(int(42.0 / step))]
            expected = bytearray(is_inked(glyph, x, y) for x in xs)
            self.assertEqual(inked_flags(glyph, y, xs), expected, f"glyph {glyph} at y {y}")
        # rows through segment ends, along horizontal strokes and at a stroke's very edge,
        # where the row touches the stroke's outline
        xs = [i / 16.0 for i in range(-64, 576)]
        for glyph, strokes in enumerate(STROKES):
            for stroke in strokes:
                for _, y in stroke:
                    for row in (y, y - 3.25, y + 3.25, y + 3.249999, y - 3.2500001):
                        expected = bytearray(is_inked(glyph, x, row) for x in xs)
                        self.assertEqual(inked_flags(glyph, row, xs), expected, (glyph, row))

    def test_every_glyph_stroke_lies_inside_its_rounded_tile(self) -> None:
        # what lets a vector canvas stroke the glyphs without clipping them to the tile
        inset = (KANA_TILE - KANA_GLYPH_BOX) / 2.0
        scale = GLYPH_GRID / KANA_GLYPH_BOX
        points = [-1.0 + 0.25 * i for i in range(109)]
        for glyph in range(GLYPH_COUNT):
            for v in points:
                xs = [(u - inset) * scale for u in points]
                for u, inked in zip(points, inked_flags(glyph, (v - inset) * scale, xs)):
                    if inked:
                        self.assertTrue(kana_inside(u, v), f"glyph {glyph} at ({u}, {v})")
        # the vector strokes: the design grid scaled into the glyph box, inset by one unit
        self.assertEqual(kana_glyph_strokes(0)[0][0], (inset + 22.0 / scale, inset + 4.0 / scale))
        shapes = draw_list(FrameCells((False,) * 296, (False,) * 3376))
        self.assertEqual(shapes.kana_stroke_width, STROKE_WIDTH * KANA_GLYPH_BOX / GLYPH_GRID)
        with self.assertRaises(IndexError):
            kana_glyph_strokes(16)


class CardLayoutTest(unittest.TestCase):
    def test_the_card_holds_306_tiles_per_band_and_eight_pilots_each(self) -> None:
        self.assertEqual((KANA_BAND_HEIGHT, CARD_HEIGHT, SQUARE_TOP), (322.0, 1668.0, 322.0))
        tiles = kana_tiles()
        self.assertEqual(len(tiles), KANA_TILE_COUNT)
        for band in range(KANA_BANDS):
            in_band = [t for t in tiles if t.band == band]
            self.assertEqual(len(in_band), 306, f"band {band}")
            self.assertEqual(sum(t.pilot is not None for t in in_band), KANA_PILOTS)
            self.assertEqual(len(kana_data_tiles(band)), 306 - KANA_PILOTS)
            for pilot, tile in enumerate(kana_pilot_tiles(band)):
                self.assertEqual(tiles[tile].pilot, pilot)
        # the bands mirror each other about the square's middle
        for tile in (t for t in tiles if t.band == 0):
            mirror = next(
                t
                for t in tiles
                if t.band == 1 and t.column == tile.column and t.row == KANA_ROWS - 1 - tile.row
            )
            self.assertLess(abs(mirror.y + KANA_TILE - CANVAS + tile.y), 1e-9)
            self.assertEqual(mirror.pilot, tile.pilot)
        # every tile centre maps back to its tile, and the square holds none
        for index, tile in enumerate(tiles):
            found = kana_tile_at(*tile.centre())
            self.assertIsNotNone(found)
            self.assertEqual(found[0], index)
            self.assertEqual(found[1:], (12.5, 12.5))
        self.assertIsNone(kana_tile_at(512.0, 512.0))
        # gutters, the margins and points beyond the lattice fall in no tile
        first = tiles[0]
        self.assertIsNone(kana_tile_at(first.x - 1.0, first.y + 5.0))
        self.assertIsNone(kana_tile_at(first.x + 5.0, -SQUARE_TOP + 1.0))
        self.assertIsNone(kana_tile_at(2.0, -100.0))
        self.assertIsNone(kana_tile_at(1022.0, -100.0))
        self.assertIsNone(kana_tile_at(512.0, CARD_HEIGHT))
        self.assertIsNone(kana_tile_at(math.nan, -100.0))
        self.assertIsNone(kana_tile_at(math.inf, -100.0))
        # no tile near a finder: the corners of the bands next to the square are empty
        self.assertIsNone(kana_tile_at(72.0, -20.0))
        with self.assertRaises(IndexError):
            kana_data_tiles(2)

    def test_tiles_are_rounded_squares_and_glyphs_are_cut_to_them(self) -> None:
        self.assertTrue(kana_inside(0.0, 12.0) and kana_inside(25.0, 3.0) and kana_inside(3.0, 0.0))
        self.assertFalse(kana_inside(0.0, 0.0) or kana_inside(25.0, 25.0) or kana_inside(-0.1, 9.0))
        self.assertTrue(
            kana_inside(3.0 - 3.0 * math.sqrt(0.5) + 1e-9, 3.0 - 3.0 * math.sqrt(0.5) + 1e-9)
        )
        # ロ is a box: its left stroke runs 5.3 units inside the tile's edge
        scale = KANA_GLYPH_BOX / GLYPH_GRID
        stroke = 1.0 + 6.0 * scale
        self.assertTrue(kana_inked(1, stroke, 12.5))
        self.assertFalse(kana_inked(1, 12.5, 12.5))
        self.assertFalse(kana_inked(1, stroke, 30.0))


def kana_words(header: LaneHeader) -> list:
    """The transmitted kana codewords (``P0``, ``K0``, ``P1``, ``K1``) of the reference tests'
    atoms under ``header``."""
    words = []
    for lane in KANA_LANES:
        if lane.kind is KanaKind.GLYPH:
            atoms = bytes(((i * 7) & 0xFF) ^ lane.band for i in range(112))
        else:
            atoms = bytes(i ^ lane.band for i in range(16))
        words.append(encode_kana_lane(lane, kana_data(header, lane, atoms)))
    return words


class KanaLaneTest(unittest.TestCase):
    def test_lane_sizes_and_ids_are_consistent(self) -> None:
        self.assertEqual((KANA_GLYPH_WORD, KANA_POLARITY_WORD), (149, 37))
        self.assertEqual(len(kana_tiles()), KANA_TILE_COUNT)
        self.assertEqual(len(kana_data_tiles(0)), KANA_DATA_TILES)
        ids = set()
        for frame in (0, 1, 65_535):
            for lane in KANA_LANES:
                for k in range(lane.atoms):
                    atom_id = kana_first_id(frame, lane) + k
                    self.assertNotIn(atom_id, ids)
                    ids.add(atom_id)
        # above every turn atom id and below the u32 limit
        self.assertGreaterEqual(kana_first_id(0, KanaLane.K0), turn_first_id(65_535, 3) + 4)
        self.assertLess(kana_first_id(65_535, KanaLane.P1), 0xFFFFFFFF - 16)
        self.assertEqual(KANA_ID_BASE, (1 << 31) + (1 << 20))
        self.assertEqual([lane.value for lane in KANA_LANES], ["P0", "K0", "P1", "K1"])
        self.assertEqual(list(KanaLane), list(KANA_LANES))
        self.assertEqual([lane.code for lane in KANA_LANES], [0x10, 0x11, 0x12, 0x13])
        self.assertEqual([lane.band for lane in KANA_LANES], [0, 0, 1, 1])
        self.assertEqual(
            [(lane.atoms, lane.data_len) for lane in KANA_LANES[:2]], [(1, 20), (7, 116)]
        )
        self.assertIs(KanaLane.of(1, KanaKind.GLYPH), KanaLane.K1)
        self.assertIs(KanaLane.of(0, KanaKind.POLARITY), KanaLane.P0)
        with self.assertRaises(ValueError):
            KanaLane.of(2, KanaKind.GLYPH)
        with self.assertRaises(ValueError):
            kana_first_id(65_536, KanaLane.P0)

    def test_whitening_is_balanced_and_distinct_per_lane(self) -> None:
        whitening = [lane.whitening() for lane in KANA_LANES]
        for lane, sequence in zip(KANA_LANES, whitening):
            self.assertEqual(len(sequence), lane.word_len)
            ones = sum(bin(byte).count("1") for byte in sequence)
            bits = len(sequence) * 8
            self.assertTrue(bits * 35 // 100 < ones < bits * 65 // 100, lane)
        self.assertNotEqual(whitening[1][:37], whitening[0])
        self.assertNotEqual(whitening[1], whitening[3])

    def test_symbols_roundtrip_through_codewords(self) -> None:
        header = LaneHeader(0x5A, 77)
        words = kana_words(header)
        symbols = pilot_symbols()
        for band in range(KANA_BANDS):
            set_kana_symbols(symbols, band, words[2 * band], words[2 * band + 1])
        cells = FrameCells((False,) * 296, (False,) * 3376).with_kana(words)
        self.assertEqual(list(cells.kana), symbols)
        for band in range(KANA_BANDS):
            polarity, glyph = (
                KanaLane.of(band, KanaKind.POLARITY),
                KanaLane.of(band, KanaKind.GLYPH),
            )
            self.assertEqual(polarity_word(symbols, band), words[2 * band])
            self.assertEqual(glyph_word(symbols, band), words[2 * band + 1])
            data, corrected = decode_kana_lane_counted(glyph, words[2 * band + 1])
            self.assertEqual(corrected, 0)
            packet = kana_packet(header, glyph, data)
            self.assertIsNotNone(packet, "check matches")
            self.assertEqual(packet.first_id, kana_first_id(77, glyph))
            self.assertEqual(
                b"".join(packet.atoms), bytes(((i * 7) & 0xFF) ^ band for i in range(112))
            )
            self.assertEqual(
                int.from_bytes(data[112:], "big"), kana_check(header, glyph, data[:112])
            )
            # the check refuses another frame, band or kind
            self.assertIsNone(kana_packet(LaneHeader(0x5A, 78), glyph, data))
            self.assertIsNone(kana_packet(LaneHeader(0x5B, 77), glyph, data))
            self.assertIsNone(kana_packet(header, KanaLane.of(1 - band, KanaKind.GLYPH), data))
            self.assertIsNone(kana_packet(header, polarity, data[:20]))
            data, _ = decode_kana_lane_counted(polarity, words[2 * band])
            self.assertEqual(
                kana_packet(header, polarity, data).atoms, (bytes(i ^ band for i in range(16)),)
            )
            # data tiles 296 and 297, beyond the polarity codeword, are light
            data_tiles = kana_data_tiles(band)
            self.assertTrue(symbols[data_tiles[296]].light and symbols[data_tiles[297]].light)
        # pilots keep their symbols
        for index, tile in enumerate(kana_tiles()):
            if tile.pilot is not None:
                self.assertEqual(symbols[index], KanaSymbol(*KANA_PILOT_SYMBOLS[tile.pilot]))
        # fifteen wrong bytes of a glyph lane are corrected (its 33 parity bytes correct
        # sixteen); a word whitened for the other band does not decode
        damaged = bytearray(words[1])
        for position in range(0, 149, 10):
            damaged[position] ^= 0x3C
        self.assertEqual(decode_kana_lane_counted(KanaLane.K0, bytes(damaged))[1], 15)
        with self.assertRaises(RsError):
            decode_kana_lane_counted(KanaLane.K1, words[1])
        with self.assertRaises(RsError):
            decode_kana_lane_counted(KanaLane.K0, words[1][:148])
        with self.assertRaises(ValueError):
            kana_data(header, KanaLane.P0, bytes(15))
        with self.assertRaises(ValueError):
            encode_kana_lane(KanaLane.K0, bytes(115))
        with self.assertRaises(ValueError):
            set_kana_symbols(symbols, 0, words[0][:36], words[1])
        with self.assertRaises(ValueError):
            cells.with_kana(words[:3])
        with self.assertRaises(ValueError):
            FrameCells((False,) * 296, (False,) * 3376, kana=((16, True),) * KANA_TILE_COUNT)
        with self.assertRaises(ValueError):
            FrameCells((False,) * 296, (False,) * 3376, kana=((0, True),) * 611)

    def test_templates_are_unit_and_tell_glyphs_apart(self) -> None:
        points = KANA_GRID * KANA_GRID
        for blur in range(len(KANA_BLURS)):
            for glyph in range(GLYPH_COUNT):
                for light in (True, False):
                    norm = reduce(
                        add,
                        (template_probe(blur, glyph, light, p % KANA_GRID, p // KANA_GRID) ** 2
                         for p in range(points)),
                    )  # fmt: skip
                    self.assertLess(abs(norm - 1.0), 1e-9)
        for glyph in range(GLYPH_COUNT):
            for light in (True, False):
                values = [
                    coarse_template_probe(glyph, light, p % KANA_COARSE_GRID, p // KANA_COARSE_GRID)
                    for p in range(KANA_COARSE_GRID * KANA_COARSE_GRID)
                ]
                self.assertLess(abs(reduce(add, (v * v for v in values)) - 1.0), 1e-9)
                self.assertLess(abs(reduce(add, values)), 1e-9)
        # at the sharpest blur no two light templates correlate above 0.8
        light = [
            [template_probe(0, glyph, True, p % KANA_GRID, p // KANA_GRID) for p in range(points)]
            for glyph in range(GLYPH_COUNT)
        ]
        for a in range(GLYPH_COUNT):
            for b in range(a + 1, GLYPH_COUNT):
                dot = reduce(add, (x * y for x, y in zip(light[a], light[b])))
                self.assertLess(dot, 0.8, f"glyphs {a} and {b}: {dot}")
        with self.assertRaises(IndexError):
            template_probe(len(KANA_BLURS), 0, True, 0, 0)
        with self.assertRaises(IndexError):
            template_probe(0, 0, True, KANA_GRID, 0)
        with self.assertRaises(IndexError):
            coarse_template_probe(16, True, 0, 0)


def card(cells: FrameCells, size: int = 512) -> Luma:
    """A card of ``cells`` ``size`` pixels wide, 2 x 2 supersampled."""
    return render_frame(cells, RenderOptions(size=size, supersample=2)).to_luma()


class BandReaderTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.encoder = StreamEncoder(payload(3_000, 3), 2)
        cls.cells = cls.encoder.cells(5)
        cls.image = card(cls.cells)
        cls.frame = decode_frame(cls.image)
        cls.header = LaneHeader(cls.encoder.meta.tag, 5)

    def test_a_clean_card_shows_every_tile_and_reads_every_kana_lane(self) -> None:
        self.assertEqual(self.frame.lanes, "D0123")
        truth = self.encoder.lane_data(5)
        results = read_kana(self.image, self.frame, self.header)
        self.assertEqual(len(results), len(KANA_LANES))
        for lane, result, data in zip(KANA_LANES, results, truth.kana):
            self.assertIsNotNone(result, lane)
            self.assertEqual(result.result.data, data, lane)
            self.assertEqual((result.result.corrected, result.result.erasures), (0, 0))
            self.assertEqual(result.header, self.header)
            packet = kana_packet(result.header, lane, result.result.data)
            self.assertEqual(packet.first_id, kana_first_id(5, lane))
        # the diagnostics see every pilot, and every data tile as drawn
        for band in range(KANA_BANDS):
            reading = observe_band(self.image, self.frame, band)
            self.assertEqual(reading.pilots, KANA_PILOTS)
            self.assertIn(reading.blur, KANA_BLURS)
            self.assertEqual(len(reading.tiles), KANA_DATA_TILES)
            for tile, read in zip(kana_data_tiles(band), reading.tiles):
                self.assertEqual(read.tile, tile)
                self.assertEqual((read.glyph, read.light), tuple(self.cells.kana[tile]), tile)
                self.assertGreater(read.glyph_lead, 0.0)
                self.assertGreater(read.light_margin, 0.0)

    def test_bands_are_skipped_and_neighbouring_frames_matched(self) -> None:
        results = read_kana(self.image, self.frame, self.header, (True, False))
        self.assertEqual([r is not None for r in results], [False, False, True, True])
        self.assertEqual(read_kana(self.image, self.frame, self.header, (True, True)), (None,) * 4)
        # a rolling shutter shows the bands of the frame after or before the square's: the
        # check tries the square's frame, the one after it and the one before it
        for shown, expected in ((6, 6), (4, 4), (7, None)):
            mixed = replace(self.cells, kana=self.encoder.cells(shown).kana)
            image = card(mixed)
            frame = decode_frame(image)
            results = read_kana(image, frame, self.header)
            for lane, result in zip(KANA_LANES, results):
                if expected is None:
                    self.assertIsNone(result, (shown, lane))
                else:
                    self.assertEqual(result.header, LaneHeader(self.header.tag, expected))
                    truth = self.encoder.lane_data(expected).kana[KANA_LANES.index(lane)]
                    self.assertEqual(result.result.data, truth)
        # another stream's tag never matches
        self.assertEqual(
            read_kana(self.image, self.frame, LaneHeader(self.header.tag ^ 1, 5)), (None,) * 4
        )
        with self.assertRaises(ValueError):
            read_kana(self.image, self.frame, self.header, (False,))
        with self.assertRaises(IndexError):
            observe_band(self.image, self.frame, 2)

    def test_bands_out_of_view_read_nothing(self) -> None:
        # the bare square: no pilot in view
        square = render_square(self.cells, RenderOptions(size=512, supersample=2)).to_luma()
        frame = decode_frame(square)
        self.assertEqual(frame.lanes, "D0123")
        self.assertEqual(read_kana(square, frame, self.header), (None,) * 4)
        self.assertIsNone(observe_band(square, frame, 0))
        # an image the pose places no tile in reads nothing either, nor does a plane whose
        # buffer does not match its size
        blank = DecodedFrame(self.frame.pose, 0, False, None)
        self.assertEqual(read_kana(Luma(8, 8), blank, self.header), (None,) * 4)
        self.assertEqual(read_kana(Luma(0, 0), blank, self.header), (None,) * 4)
        broken = Luma(self.image.width, self.image.height, self.image.data)
        broken.data = bytes(5)
        self.assertEqual(read_kana(broken, self.frame, self.header), (None,) * 4)


class CardRenderTest(unittest.TestCase):
    def test_katakana_tiles_show_their_polarity_and_glyph(self) -> None:
        base = StreamEncoder(payload(200, 4), 1).cells(2)
        symbols = tuple(KanaSymbol(i % 16, i % 3 != 0) for i in range(KANA_TILE_COUNT))
        frame = replace(base, kana=symbols)
        image = render_frame(frame, RenderOptions(size=1024, supersample=1))
        self.assertEqual((image.width, image.height), (1024, 1668))
        palette = Palette()
        for index, tile in enumerate(kana_tiles()):
            glyph, light = symbols[index]
            # the pixel centres of the tile's square, against the definition
            x0 = math.ceil(tile.x)
            y0 = math.ceil(tile.y + SQUARE_TOP)
            for dx, dy in ((3, 3), (12, 12), (8, 17), (20, 6)):
                px, py = x0 + dx, y0 + dy
                u = px + 0.5 - tile.x
                v = py + 0.5 - SQUARE_TOP - tile.y
                inked = kana_inked(glyph, u, v)
                if inked:
                    expected = palette.ink if light else palette.pink
                else:
                    expected = palette.light if light else palette.background
                at = (py * 1024 + px) * 3
                self.assertEqual(
                    tuple(image.data[at : at + 3]), expected, f"tile {index} at ({dx}, {dy})"
                )

    def test_the_card_is_the_square_with_a_band_above_and_below(self) -> None:
        self.assertEqual(
            [card_height_px(size) for size in (512, 300, 777, 1024, 1)], [834, 489, 1266, 1668, 2]
        )


class UprightSessionTest(unittest.TestCase):
    def test_an_upright_camera_reads_both_bands_once_per_shown_frame(self) -> None:
        data = payload(3000, 3)
        # a phone held upright delivers landscape frames with the card turned a quarter
        forward = camera_homography(640, 480, 0.79, 92.0, tilt=0.15)

        def shoot(frame: int, seed: int) -> Luma:
            return capture(rendered(data, 2, frame, 512, 2).to_luma(), forward, 640, 480, seed)

        session = ScanSession()
        first = session.push(shoot(1, 1), 0)
        self.assertEqual((first.lanes, first.kana, first.turned), ("D0123", "P0K0P1K1", ""))
        # a second capture of the same shown frame tries no band again, so its turn lanes
        # are read
        again = session.push(shoot(1, 2), 33)
        self.assertEqual((again.kana, again.turned), ("", "0123"))
        following = session.push(shoot(2, 3), 83)
        self.assertEqual((following.kana, following.turned), ("P0K0P1K1", ""))
        stats = session.stats()
        self.assertEqual((stats.kana_tries, stats.kana_lanes), (4, 8))
        self.assertEqual((stats.turn_tries, stats.turn_lanes), (4, 4))
        tag = StreamEncoder(data, 2).meta.tag
        self.assertEqual(
            list(session._kana_tried)[-2:], [(LaneHeader(tag, 2), 0), (LaneHeader(tag, 2), 1)]
        )
        # the session remembers the last KANA_MEMORY bands it tried, the two of each of the
        # last six shown frames
        self.assertEqual((KANA_MEMORY, KANA_BACKOFF), (12, 8))
        session._kana_tried.clear()
        session._kana_tried.extend((LaneHeader(0, frame), 0) for frame in range(KANA_MEMORY))
        self.assertEqual(session.push(shoot(3, 4), 166).kana, "P0K0P1K1")
        self.assertEqual(len(session._kana_tried), KANA_MEMORY)
        self.assertEqual(session._kana_tried[0], (LaneHeader(0, 2), 0))
        self.assertEqual(session._kana_tried[-1], (LaneHeader(tag, 3), 1))
        # a reset forgets the bands tried and the back-off
        session._kana_misses, session._kana_wait = 3, 3
        session.reset()
        self.assertEqual(
            (len(session._kana_tried), session._kana_misses, session._kana_wait), (0, 0, 0)
        )


if __name__ == "__main__":
    unittest.main()
