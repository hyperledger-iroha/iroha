# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Software renderer and the vector draw list."""

from __future__ import annotations

import math
import unittest
import xml.etree.ElementTree as ElementTree

import petal_test_support  # noqa: F401  (puts src on sys.path)

from iroha_petal.glyphs import STROKES
from iroha_petal.lanes import D_DATA, K_DATA, P_DATA, FrameCells, Lane, encode_lane
from iroha_petal.layout import (
    FINDER_CENTERS,
    RING_SLOTS,
    TILE_COUNT,
    TOTAL_SLOTS,
    ring_offset,
    slot_center,
    tile_center,
)
from iroha_petal.render import Palette, RenderOptions, draw_list, render_frame


def cells(seed: int) -> FrameCells:
    p = bytes((b * 31 + seed) & 0xFF for b in range(P_DATA))
    k = bytes(((b * 17) & 0xFF) ^ seed for b in range(K_DATA))
    d = bytes(((b * 13) & 0xFF) ^ seed for b in range(D_DATA))
    return FrameCells.from_words(
        encode_lane(Lane.P, p), encode_lane(Lane.K, k), encode_lane(Lane.D, d)
    )


class RenderTest(unittest.TestCase):
    def test_finders_are_solid_blossoms_and_corners_are_otherwise_black(self) -> None:
        image = render_frame(cells(1), RenderOptions(size=256, supersample=2))

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

    def test_light_tiles_are_bright_and_dark_tiles_are_mostly_black(self) -> None:
        frame = cells(2)
        image = render_frame(frame, RenderOptions(size=512, supersample=2))
        scale = 512.0 / 1024.0
        light, dark = [], []
        for tile in range(TILE_COUNT):
            cx, cy = tile_center(tile)
            x0, y0 = int((cx - 10.0) * scale), int((cy - 10.0) * scale)
            total = sum(
                image.data[((y0 + j) * 512 + x0 + i) * 3] for j in range(10) for i in range(10)
            )
            (light if frame.light[tile] else dark).append(total / 100.0)
        light_mean = sum(light) / len(light)
        dark_mean = sum(dark) / len(dark)
        # bold glyphs ink the middle of every tile, so only the ordering is stable
        self.assertGreater(light_mean, dark_mean + 25.0)

    def test_lit_dots_are_drawn_and_unlit_slots_are_black(self) -> None:
        frame = cells(3)
        image = render_frame(frame, RenderOptions(size=1024, supersample=1))
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

    def test_options_are_validated_and_the_palette_is_applied(self) -> None:
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=0))
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=64, supersample=5))
        palette = Palette(background=(10, 20, 30), light=(200, 100, 50))
        frame = cells(4)
        image = render_frame(frame, RenderOptions(size=128, supersample=1, palette=palette))
        self.assertEqual(tuple(image.data[0:3]), (10, 20, 30))
        # the finder core is filled with the light colour
        at = (9 * 128 + 9) * 3
        self.assertEqual(tuple(image.data[at : at + 3]), (200, 100, 50))


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
        self.assertEqual(len(shapes.tiles), TILE_COUNT)
        palette = Palette()
        for tile in shapes.tiles:
            cx, cy = tile_center(tile.index)
            self.assertEqual((tile.x, tile.y, tile.size), (cx - 12.5, cy - 12.5, 25.0))
            self.assertEqual(tile.corner_radius, 3.0)
            self.assertEqual(tile.light, frame.light[tile.index])
            self.assertEqual(tile.glyph, frame.glyph[tile.index])
            self.assertEqual(tile.fill, palette.light if tile.light else None)
            self.assertEqual(tile.glyph_color, palette.ink if tile.light else palette.pink)
            self.assertEqual(tile.glyph_box, (cx - 11.5, cy - 11.5, 23.0))
            self.assertAlmostEqual(tile.stroke_width, 6.5 * 23.0 / 32.0)
            self.assertEqual(len(tile.strokes), len(STROKES[tile.glyph]))
            gx, gy, side = tile.glyph_box
            for stroke in tile.strokes:
                for x, y in stroke:
                    self.assertTrue(gx <= x <= gx + side and gy <= y <= gy + side)
        lit = [i for i, on in enumerate(frame.dots) if on]
        self.assertEqual(len(shapes.dots), len(lit))
        for dot, flat in zip(shapes.dots, lit):
            ring = 0 if flat < 80 else 1 if flat < 172 else 2
            x, y = slot_center(ring, flat - ring_offset(ring))
            self.assertLess(math.hypot(dot.x - x, dot.y - y), 1e-3)
            self.assertEqual(dot.radius, 11.0)

    def test_svg_output_is_well_formed(self) -> None:
        frame = cells(6)
        shapes = draw_list(frame)
        svg = shapes.to_svg(512)
        root = ElementTree.fromstring(svg)
        namespace = "{http://www.w3.org/2000/svg}"
        self.assertEqual(root.tag, namespace + "svg")
        self.assertEqual(root.get("width"), "512")
        circles = root.findall(f".//{namespace}circle")
        self.assertEqual(len(circles), 4 * 11 + len(shapes.dots))
        rects = root.findall(f".//{namespace}rect")
        self.assertEqual(len(rects), 1 + sum(frame.light))
        polylines = root.findall(f".//{namespace}polyline")
        self.assertEqual(len(polylines), sum(len(STROKES[g]) for g in frame.glyph))


if __name__ == "__main__":
    unittest.main()
