# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Software renderer and the vector draw list."""

from __future__ import annotations

import math
import unittest
import xml.etree.ElementTree as ElementTree

from petal_test_support import payload

from iroha_petal._numeric import round_half_away
from iroha_petal.crc import crc32c
from iroha_petal.kana import KANA_LANES, encode_kana_lane, kana_data
from iroha_petal.lanes import D_DATA, FIELD_DATA, FIELD_LANES, FrameCells, Lane, encode_lane
from iroha_petal.layout import (
    BACKDROP_RADIUS,
    BAND_COUNT,
    BAND_OFFSETS,
    BAND_SLOTS,
    CANVAS,
    CARD_HEIGHT,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    FINDER_OUTER,
    KANA_CORNER,
    KANA_TILE,
    MASK,
    PETAL_HALF_LENGTH,
    RING_RADII,
    RING_SLOTS,
    SQUARE_TOP,
    TOTAL_SLOTS,
    band_phase,
    band_radius,
    cell_angle,
    cell_center,
    cell_light,
    finder_lit,
    in_mask,
    kana_inked,
    kana_inside,
    kana_tile_at,
    kana_tiles,
    petal_lit,
    ring_offset,
    slot_center,
)
from iroha_petal.render import (
    Palette,
    RenderOptions,
    _svg_number,
    card_height_px,
    draw_list,
    kana_glyph_strokes,
    render_frame,
    render_square,
)
from iroha_petal.stream import LaneHeader, StreamEncoder
from iroha_petal.turns import TURN_ATOMS, encode_turn_lane, turn_angle, turn_data


def cells(seed: int) -> FrameCells:
    """Cells of arbitrary lane data, the carriers turned to show arbitrary turn lanes and the
    katakana tiles arbitrary kana lanes."""
    d = bytes(((b * 13) & 0xFF) ^ seed for b in range(D_DATA))
    fields = [
        encode_lane(lane, bytes(((b * 17 + s) & 0xFF) ^ seed for b in range(FIELD_DATA)))
        for s, lane in enumerate(FIELD_LANES)
    ]
    turns = [
        encode_turn_lane(
            s,
            turn_data(
                LaneHeader(seed, 9), s, bytes(((b * 29 + s) & 0xFF) ^ seed for b in range(64))
            ),
        )
        for s in range(len(FIELD_LANES))
    ]
    assert TURN_ATOMS * 16 == 64
    kana = [
        encode_kana_lane(
            lane,
            kana_data(
                LaneHeader(seed, 9),
                lane,
                bytes(((b * 31 + k) & 0xFF) ^ seed for b in range(lane.atoms * 16)),
            ),
        )
        for k, lane in enumerate(KANA_LANES)
    ]
    return FrameCells.from_words(encode_lane(Lane.D, d), fields).with_turns(turns).with_kana(kana)


def _round(value: float) -> float:
    return -round_half_away(-value) if value < 0.0 else round_half_away(value)


def literal_shade(frame: FrameCells, palette: Palette, x: float, y: float):
    """The reference renderer's ``Shader::shade``, line by line."""
    for fx, fy in FINDER_CENTERS:
        dx, dy = x - fx, y - fy
        if math.sqrt(dx * dx + dy * dy) <= FINDER_OUTER:
            return palette.light if finder_lit(dx, dy) else palette.background
    dx, dy = x - 512.0, y - 512.0
    radius = math.sqrt(dx * dx + dy * dy)
    theta = math.atan2(dy, dx)
    if theta < 0.0:
        theta += math.tau
    for ring in range(3):
        ring_radius = RING_RADII[ring]
        if abs(radius - ring_radius) > DOT_RADIUS:
            continue
        slots = RING_SLOTS[ring]
        slot = int(_round(theta / math.tau * slots)) % slots
        if not frame.dots[ring_offset(ring) + slot]:
            continue
        angle = math.tau * slot / slots
        px, py = ring_radius * math.cos(angle), ring_radius * math.sin(angle)
        if math.sqrt((dx - px) * (dx - px) + (dy - py) * (dy - py)) <= DOT_RADIUS:
            return palette.pink
    if radius <= BACKDROP_RADIUS:
        nearest = _round((radius - 16.0) / 11.0)
        for band in (nearest - 1.0, nearest, nearest + 1.0):
            if band < 0.0 or band >= BAND_COUNT:
                continue
            b = int(band)
            if abs(radius - band_radius(b)) > PETAL_HALF_LENGTH:
                continue
            slots = float(BAND_SLOTS[b])
            around = int(_round(theta / math.tau * slots - band_phase(b)))
            for slot in (around - 1, around, around + 1):
                cell = BAND_OFFSETS[b] + slot % BAND_SLOTS[b]
                if not frame.petals[cell]:
                    continue
                cx, cy = cell_center(cell)
                if petal_lit(x - cx, y - cy, cell_angle(cell) + turn_angle(frame.turns[cell])):
                    return palette.light if cell_light(cell) else palette.pink
        if in_mask(x, y):
            return palette.backdrop
    return palette.background


def literal_kana(frame: FrameCells, palette: Palette, x: float, y: float):
    """The reference renderer's ``Shader::kana``, line by line."""
    found = kana_tile_at(x, y)
    if found is None:
        return palette.background
    tile, u, v = found
    if not kana_inside(u, v):
        return palette.background
    glyph, light = frame.kana[tile]
    inked = kana_inked(glyph, u, v)
    if light:
        return palette.ink if inked else palette.light
    return palette.pink if inked else palette.background


def literal_pixel(
    frame: FrameCells, palette: Palette, px: int, py: int, size: int, s: int, top: float = 0.0
):
    """One pixel of the reference renderer: the rounded mean of its ``s x s`` samples, the
    first row ``top`` canvas units above the square's top edge."""
    unit = 1024.0 / size
    acc = [0, 0, 0]
    for sy in range(s):
        for sx in range(s):
            x = (px + (sx + 0.5) / s) * unit
            y = (py + (sy + 0.5) / s) * unit - top
            if 0.0 <= y < CANVAS:
                colour = literal_shade(frame, palette, x, y)
            else:
                colour = literal_kana(frame, palette, x, y)
            for k in range(3):
                acc[k] += colour[k]
    n = s * s
    return tuple((value + n // 2) // n for value in acc)


class RenderTest(unittest.TestCase):
    def test_finders_are_solid_blossoms_and_corners_are_otherwise_black(self) -> None:
        image = render_square(cells(1), RenderOptions(size=256, supersample=2))

        def at(x: int, y: int) -> int:
            return image.data[(y * 256 + x) * 3]

        scale = 256.0 / 1024.0
        fx, fy = FINDER_CENTERS[0][0] * scale, FINDER_CENTERS[0][1] * scale
        self.assertGreater(at(int(fx), int(fy)), 200, "core must be lit")
        self.assertGreater(at(int(fx), int(fy - 34.0 * scale)), 200, "upper petal must be lit")
        self.assertGreater(
            at(int(fx + 20.0 * scale), int(fy + 20.0 * scale)), 200, "blossom body must be lit"
        )
        self.assertEqual(at(2, 255), 0)

    def test_lit_petals_are_drawn_in_their_colour_and_dark_cells_show_the_backdrop(self) -> None:
        frame = cells(2)
        image = render_square(frame, RenderOptions(size=1024, supersample=1))
        palette = Palette()
        for cell in range(FIELD_CELLS):
            x, y = cell_center(cell)
            at = (int(y) * 1024 + int(x)) * 3
            pixel = tuple(image.data[at : at + 3])
            if frame.petals[cell]:
                expected = palette.light if cell_light(cell) else palette.pink
            elif in_mask(math.floor(x) + 0.5, math.floor(y) + 0.5):
                expected = palette.backdrop
            else:
                expected = palette.background
            self.assertEqual(pixel, expected, f"cell {cell}")

    def test_lit_dots_are_drawn_and_unlit_slots_are_black(self) -> None:
        frame = cells(3)
        image = render_square(frame, RenderOptions(size=1024, supersample=1))
        checked = 0
        for ring, slots in enumerate(RING_SLOTS):
            for slot in range(slots):
                x, y = slot_center(ring, slot)
                value = image.data[(int(y) * 1024 + int(x)) * 3]
                if frame.dots[ring_offset(ring) + slot]:
                    self.assertGreater(value, 150, f"ring {ring} slot {slot} should be lit")
                else:
                    self.assertEqual(value, 0, f"ring {ring} slot {slot} should be dark")
                checked += 1
        self.assertEqual(checked, TOTAL_SLOTS)

    def test_the_painter_matches_a_literal_port_of_the_reference_shader(self) -> None:
        # the renderer paints shape by shape and tile row by tile row; the reference shades
        # sample by sample in priority order. Rows through the finders, the rings, the
        # backdrop and the turned petals of every band, and through the katakana tiles of
        # both bands (their edges, rounded corners and glyph strokes), must agree pixel for
        # pixel, on the bare square and on the card.
        frame = cells(4)
        self.assertEqual(set(frame.turns), {0, 1, 2, 3})
        self.assertEqual({light for _, light in frame.kana}, {True, False})
        palette = Palette()
        for size, s, rows, card_rows in (
            (1024, 1, (70, 300, 512, 515, 700), (0, 10, 11, 13, 160, 298, 1370, 1384, 1667)),
            (768, 2, (54, 200, 383), (8, 9, 117, 1030, 1250)),
            (512, 3, (36, 256, 400), (4, 6, 79, 680, 833)),
        ):
            image = render_square(frame, RenderOptions(size=size, supersample=s))
            for py in rows:
                for px in range(size):
                    at = (py * size + px) * 3
                    self.assertEqual(
                        tuple(image.data[at : at + 3]),
                        literal_pixel(frame, palette, px, py, size, s),
                        f"size {size}x{s}: pixel ({px}, {py})",
                    )
            card = render_frame(frame, RenderOptions(size=size, supersample=s))
            self.assertEqual((card.width, card.height), (size, card_height_px(size)))
            top = SQUARE_TOP * size / CANVAS
            for py in card_rows + tuple(round(row + top) for row in rows):
                for px in range(size):
                    at = (py * size + px) * 3
                    self.assertEqual(
                        tuple(card.data[at : at + 3]),
                        literal_pixel(frame, palette, px, py, size, s, SQUARE_TOP),
                        f"card {size}x{s}: pixel ({px}, {py})",
                    )

    def test_software_renders_are_byte_identical_to_the_reference(self) -> None:
        # CRC-32C of the Rust reference `render` output (the whole card) for frames of a
        # 700-byte stream (turned petals and katakana tiles included)
        encoder = StreamEncoder(payload(700, 2), 2)
        for frame, size, s, crc in (
            (0, 512, 2, 0xFBDF7940),
            (1, 300, 3, 0xB52FB2C1),
            (6, 777, 4, 0xCA8B70B1),
        ):
            rgb = render_frame(encoder.cells(frame), RenderOptions(size=size, supersample=s))
            self.assertEqual((rgb.width, rgb.height), (size, card_height_px(size)))
            self.assertEqual(crc32c(rgb.data), crc, f"frame {frame} at {size} px, {s}x")

    def test_the_square_of_a_card_is_the_bare_square_where_the_samples_agree(self) -> None:
        # at 512 px a canvas unit is half a pixel, so the card's samples below the band are
        # exactly the bare square's: its rows are the square's rows
        frame = cells(5)
        options = RenderOptions(size=512, supersample=2)
        card = render_frame(frame, options)
        square = render_square(frame, options)
        self.assertEqual(card.height, 834)
        start = 161 * 512 * 3
        self.assertEqual(card.data[start : start + 512 * 512 * 3], square.data)

    def test_options_are_validated_and_the_palette_is_applied(self) -> None:
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=0))
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=64, supersample=5))
        with self.assertRaises(ValueError):
            render_square(cells(1), RenderOptions(size=64, supersample=0))
        palette = Palette(
            background=(10, 20, 30), light=(200, 100, 50), backdrop=(1, 2, 3), ink=(7, 8, 9)
        )
        frame = cells(4)
        image = render_frame(frame, RenderOptions(size=128, supersample=1, palette=palette))
        self.assertEqual((image.width, image.height), (128, 209))
        self.assertEqual(tuple(image.data[0:3]), (10, 20, 30))
        # the finder core is filled with the light colour
        at = ((9 + 40) * 128 + 9) * 3
        self.assertEqual(tuple(image.data[at : at + 3]), (200, 100, 50))
        # the backdrop colour shows where the 天 has no lit petal, the ink on light tiles
        self.assertIn(b"\x01\x02\x03", bytes(image.data))
        self.assertIn(b"\x07\x08\x09", bytes(image.data))
        square = render_square(frame, RenderOptions(size=128, supersample=1, palette=palette))
        self.assertEqual((square.width, square.height), (128, 128))
        self.assertEqual(tuple(square.data[(9 * 128 + 9) * 3 :][:3]), (200, 100, 50))
        self.assertNotIn(b"\x07\x08\x09", bytes(square.data))


class DrawListTest(unittest.TestCase):
    def test_draw_list_describes_every_shape(self) -> None:
        frame = cells(5)
        shapes = draw_list(frame)
        self.assertEqual(shapes.canvas, 1024.0)
        self.assertEqual(len(shapes.finders), 4)
        for finder, (fx, fy) in zip(shapes.finders, FINDER_CENTERS):
            self.assertEqual(finder.center, (fx, fy))
            self.assertEqual((finder.core.x, finder.core.y, finder.core.radius), (fx, fy, 12.0))
            self.assertEqual(len(finder.petals), 5)
            self.assertEqual(len(finder.notches), 5)
            up = finder.petals[0]
            self.assertAlmostEqual(up.x, fx, places=9)
            self.assertAlmostEqual(up.y, fy - 34.0, places=9)
            self.assertEqual(up.radius, 26.0)
            self.assertAlmostEqual(finder.notches[0].y, fy - 60.0, places=9)
            self.assertEqual(finder.notches[0].radius, 6.0)
        # the backdrop: every `#` lattice cell, clipped to the disc of radius 390
        self.assertEqual(len(shapes.backdrop), sum(line.count("#") for line in MASK))
        first = shapes.backdrop[0]
        self.assertEqual((first.x, first.y, first.size), (182.0 + 33.0 * 4, 182.0, 33.0))
        clip = shapes.backdrop_clip
        self.assertEqual((clip.x, clip.y, clip.radius), (512.0, 512.0, 390.0))
        # the lit petals, in cell order, along their axis (the outward direction plus the
        # turn), with their colour class
        palette = Palette()
        lit = [cell for cell in range(FIELD_CELLS) if frame.petals[cell]]
        self.assertEqual([petal.index for petal in shapes.petals], lit)
        turned = 0
        for petal in shapes.petals:
            self.assertEqual((petal.x, petal.y), cell_center(petal.index))
            turn = frame.turns[petal.index]
            self.assertEqual(petal.angle, cell_angle(petal.index) + turn_angle(turn))
            turned += turn != 0
            self.assertEqual(petal.light, cell_light(petal.index))
            self.assertEqual(petal.color, palette.light if petal.light else palette.pink)
        self.assertGreater(turned, 4 * 360 * 6 // 10)
        self.assertAlmostEqual(shapes.petal_half_length, 6.0, places=6)
        self.assertAlmostEqual(shapes.petal_half_width, 3.73, places=6)
        self.assertAlmostEqual(shapes.petal_notch_radius, 1.12, places=6)
        self.assertAlmostEqual(shapes.petal_notch_distance, 6.24, places=6)
        # the lit dots
        lit_dots = [i for i, on in enumerate(frame.dots) if on]
        self.assertEqual(len(shapes.dots), len(lit_dots))
        for dot, flat in zip(shapes.dots, lit_dots):
            ring = 0 if flat < 88 else 1 if flat < 188 else 2
            x, y = slot_center(ring, flat - ring_offset(ring))
            self.assertLess(math.hypot(dot.x - x, dot.y - y), 1e-3)
            self.assertEqual(dot.radius, 9.0)
        # the card around the square and its katakana tiles: every tile in order, with the
        # cells' symbols, as the card shows them
        self.assertEqual(
            (shapes.canvas, shapes.card_height, shapes.square_top),
            (1024.0, CARD_HEIGHT, SQUARE_TOP),
        )
        self.assertEqual((shapes.kana_tile, shapes.kana_corner), (KANA_TILE, KANA_CORNER))
        self.assertEqual(shapes.kana_stroke_width, 6.5 * 23.0 / 32.0)
        self.assertEqual(len(shapes.kana_tiles), len(kana_tiles()))
        card = render_frame(frame, RenderOptions(size=1024, supersample=1))
        top = int(SQUARE_TOP)
        for index, (shape, tile) in enumerate(zip(shapes.kana_tiles, kana_tiles())):
            self.assertEqual((shape.index, shape.x, shape.y), (index, tile.x, tile.y))
            self.assertEqual((shape.glyph, shape.light), tuple(frame.kana[index]))
            self.assertEqual(shape.glyph_color, palette.ink if shape.light else palette.pink)
            # the pixel at the tile centre is the tile's fill or its glyph
            px, py = math.floor(tile.x + 12.5), math.floor(tile.y + 12.5)
            u, v = px + 0.5 - tile.x, py + 0.5 - tile.y
            if kana_inked(shape.glyph, u, v):
                expected = shape.glyph_color
            else:
                expected = palette.light if shape.light else palette.background
            at = ((py + top) * 1024 + px) * 3
            self.assertEqual(tuple(card.data[at : at + 3]), expected, f"tile {index}")
        # the strokes in tile units: the design grid scaled into the glyph box
        for glyph in range(16):
            for stroke in kana_glyph_strokes(glyph):
                for u, v in stroke:
                    self.assertTrue(1.0 <= u <= 24.0 and 1.0 <= v <= 24.0, (glyph, u, v))

    def test_svg_output_is_well_formed(self) -> None:
        frame = cells(6)
        shapes = draw_list(frame)
        svg = shapes.to_svg(512)
        root = ElementTree.fromstring(svg)
        namespace = "{http://www.w3.org/2000/svg}"
        self.assertEqual(root.tag, namespace + "svg")
        # the whole card, its square in square coordinates
        self.assertEqual((root.get("width"), root.get("height")), ("512", "834"))
        self.assertEqual(root.get("viewBox"), "0 -322 1024 1668")
        circles = root.findall(f".//{namespace}circle")
        # finders (core, five petals, five notches), lit dots and the backdrop clip
        self.assertEqual(len(circles), 4 * 11 + len(shapes.dots) + 1)
        rects = root.findall(f".//{namespace}rect")
        light_tiles = [tile for tile in shapes.kana_tiles if tile.light]
        self.assertEqual(len(rects), 1 + len(shapes.backdrop) + len(light_tiles))
        self.assertEqual((rects[0].get("y"), rects[0].get("height")), ("-322", "1668"))
        # a light tile is a rounded square filled light; every glyph is stroked over its tile
        for rect, tile in zip(rects[1 + len(shapes.backdrop) :], light_tiles):
            self.assertEqual(
                (rect.get("x"), rect.get("y"), rect.get("width"), rect.get("rx")),
                (_svg_number(tile.x), _svg_number(tile.y), "25", "3"),
            )
        glyph_uses = [use for use in root.iter(f"{namespace}use") if use.get("href") != "#petal"]
        self.assertEqual(len(glyph_uses), len(shapes.kana_tiles))
        for use, tile in zip(glyph_uses, shapes.kana_tiles):
            self.assertEqual(use.get("href"), f"#kana{tile.glyph}")
            self.assertEqual(
                (use.get("x"), use.get("y")), (_svg_number(tile.x), _svg_number(tile.y))
            )
            self.assertEqual(use.get("stroke"), "#14040e" if tile.light else "#f5afd0")
        for glyph in {tile.glyph for tile in shapes.kana_tiles}:
            group = root.find(f".//{namespace}g[@id='kana{glyph}']")
            lines = group.findall(f"{namespace}polyline")
            self.assertEqual(len(lines), len(kana_glyph_strokes(glyph)))
        # every petal is the notched outline, moved to its cell and turned to its axis
        outline = root.find(f".//{namespace}defs/{namespace}path")
        self.assertEqual(outline.get("id"), "petal")
        self.assertEqual(
            outline.get("d"),
            "M 5.7724 1.0177 A 6 3.73 0 1 1 5.7724 -1.0177 A 1.12 1.12 0 0 0 5.7724 1.0177 Z",
        )
        uses = [use for use in root.iter(f"{namespace}use") if use.get("href") == "#petal"]
        self.assertEqual(len(uses), sum(frame.petals))
        # the light petals come first, in cell order
        ordered = [petal for petal in shapes.petals if petal.light]
        ordered += [petal for petal in shapes.petals if not petal.light]
        for use, petal in zip(uses, ordered):
            self.assertEqual(use.get("href"), "#petal")
            self.assertEqual(
                use.get("transform"),
                f"translate({_svg_number(petal.x)} {_svg_number(petal.y)}) "
                f"rotate({_svg_number(math.degrees(petal.angle))})",
            )

    def test_the_notched_outline_is_the_petal_of_the_reference(self) -> None:
        # the outline leaves the ellipse where the notch cuts it: both on the ellipse and on
        # the notch's circle
        shapes = draw_list(cells(7))
        parts = shapes.petal_outline().split()
        x, y = float(parts[1]), float(parts[2])
        a, b = shapes.petal_half_length, shapes.petal_half_width
        self.assertAlmostEqual((x / a) ** 2 + (y / b) ** 2, 1.0, places=3)
        r, d = shapes.petal_notch_radius, shapes.petal_notch_distance
        self.assertAlmostEqual(math.hypot(x - d, y), r, places=3)
        # points inside the outline are lit by the reference's predicate, the notch is not
        self.assertTrue(petal_lit(5.0, 0.0, 0.0))
        self.assertFalse(petal_lit(5.5, 0.0, 0.0))
        self.assertTrue(petal_lit(-5.5, 0.0, 0.0))


if __name__ == "__main__":
    unittest.main()
