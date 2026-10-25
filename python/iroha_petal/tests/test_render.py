# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Software renderer and the vector draw list."""

from __future__ import annotations

import math
import unittest
import xml.etree.ElementTree as ElementTree

import petal_test_support  # noqa: F401  (puts src on sys.path)

from iroha_petal._numeric import round_half_away
from iroha_petal.lanes import D_DATA, FIELD_DATA, FIELD_LANES, FrameCells, Lane, encode_lane
from iroha_petal.layout import (
    BACKDROP_RADIUS,
    BAND_COUNT,
    BAND_OFFSETS,
    BAND_SLOTS,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    FINDER_OUTER,
    MASK,
    PETAL_HALF_LENGTH,
    RING_RADII,
    RING_SLOTS,
    TOTAL_SLOTS,
    band_phase,
    band_radius,
    cell_angle,
    cell_center,
    cell_light,
    finder_lit,
    in_mask,
    petal_lit,
    ring_offset,
    slot_center,
)
from iroha_petal.render import Palette, RenderOptions, draw_list, render_frame


def cells(seed: int) -> FrameCells:
    d = bytes(((b * 13) & 0xFF) ^ seed for b in range(D_DATA))
    fields = [
        encode_lane(lane, bytes(((b * 17 + s) & 0xFF) ^ seed for b in range(FIELD_DATA)))
        for s, lane in enumerate(FIELD_LANES)
    ]
    return FrameCells.from_words(encode_lane(Lane.D, d), fields)


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
                if petal_lit(x - cx, y - cy, cell_angle(cell)):
                    return palette.light if cell_light(cell) else palette.pink
        if in_mask(x, y):
            return palette.backdrop
    return palette.background


def literal_pixel(frame: FrameCells, palette: Palette, px: int, py: int, size: int, s: int):
    """One pixel of the reference renderer: the rounded mean of its ``s x s`` samples."""
    unit = 1024.0 / size
    acc = [0, 0, 0]
    for sy in range(s):
        for sx in range(s):
            x = (px + (sx + 0.5) / s) * unit
            y = (py + (sy + 0.5) / s) * unit
            colour = literal_shade(frame, palette, x, y)
            for k in range(3):
                acc[k] += colour[k]
    n = s * s
    return tuple((value + n // 2) // n for value in acc)


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

    def test_lit_petals_are_drawn_in_their_colour_and_dark_cells_show_the_backdrop(self) -> None:
        frame = cells(2)
        image = render_frame(frame, RenderOptions(size=1024, supersample=1))
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

    def test_the_painter_matches_a_literal_port_of_the_reference_shader(self) -> None:
        # the renderer paints shape by shape; the reference shades sample by sample in
        # priority order. Rows through the finders, the rings, the backdrop and the petals of
        # every band must agree pixel for pixel.
        frame = cells(4)
        palette = Palette()
        for size, s, rows in (
            (1024, 1, (70, 300, 512, 515, 700)),
            (768, 2, (54, 200, 383)),
            (512, 3, (36, 256, 400)),
        ):
            image = render_frame(frame, RenderOptions(size=size, supersample=s))
            for py in rows:
                for px in range(size):
                    at = (py * size + px) * 3
                    self.assertEqual(
                        tuple(image.data[at : at + 3]),
                        literal_pixel(frame, palette, px, py, size, s),
                        f"size {size}x{s}: pixel ({px}, {py})",
                    )

    def test_options_are_validated_and_the_palette_is_applied(self) -> None:
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=0))
        with self.assertRaises(ValueError):
            render_frame(cells(1), RenderOptions(size=64, supersample=5))
        palette = Palette(background=(10, 20, 30), light=(200, 100, 50), backdrop=(1, 2, 3))
        frame = cells(4)
        image = render_frame(frame, RenderOptions(size=128, supersample=1, palette=palette))
        self.assertEqual(tuple(image.data[0:3]), (10, 20, 30))
        # the finder core is filled with the light colour
        at = (9 * 128 + 9) * 3
        self.assertEqual(tuple(image.data[at : at + 3]), (200, 100, 50))
        # the backdrop colour shows where the 天 has no lit petal
        self.assertIn(b"\x01\x02\x03", bytes(image.data))


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
        # the lit petals, in cell order, with their outward angle and colour class
        palette = Palette()
        lit = [cell for cell in range(FIELD_CELLS) if frame.petals[cell]]
        self.assertEqual([petal.index for petal in shapes.petals], lit)
        for petal in shapes.petals:
            self.assertEqual((petal.x, petal.y), cell_center(petal.index))
            self.assertEqual(petal.angle, cell_angle(petal.index))
            self.assertEqual(petal.light, cell_light(petal.index))
            self.assertEqual(petal.color, palette.light if petal.light else palette.pink)
        self.assertAlmostEqual(shapes.petal_half_length, 5.2, places=6)
        self.assertAlmostEqual(shapes.petal_half_width, 4.3, places=6)
        self.assertAlmostEqual(shapes.petal_notch_radius, 1.3, places=6)
        self.assertAlmostEqual(shapes.petal_notch_distance, 5.4, places=6)
        # the lit dots
        lit_dots = [i for i, on in enumerate(frame.dots) if on]
        self.assertEqual(len(shapes.dots), len(lit_dots))
        for dot, flat in zip(shapes.dots, lit_dots):
            ring = 0 if flat < 88 else 1 if flat < 188 else 2
            x, y = slot_center(ring, flat - ring_offset(ring))
            self.assertLess(math.hypot(dot.x - x, dot.y - y), 1e-3)
            self.assertEqual(dot.radius, 9.0)

    def test_svg_output_is_well_formed(self) -> None:
        frame = cells(6)
        shapes = draw_list(frame)
        svg = shapes.to_svg(512)
        root = ElementTree.fromstring(svg)
        namespace = "{http://www.w3.org/2000/svg}"
        self.assertEqual(root.tag, namespace + "svg")
        self.assertEqual(root.get("width"), "512")
        circles = root.findall(f".//{namespace}circle")
        # finders (core, five petals, five notches), lit dots and the backdrop clip
        self.assertEqual(len(circles), 4 * 11 + len(shapes.dots) + 1)
        rects = root.findall(f".//{namespace}rect")
        self.assertEqual(len(rects), 1 + len(shapes.backdrop))
        ellipses = root.findall(f".//{namespace}ellipse")
        self.assertEqual(len(ellipses), sum(frame.petals))
        self.assertEqual(ellipses[0].get("rx"), "5.2")
        self.assertEqual(ellipses[0].get("ry"), "4.3")


if __name__ == "__main__":
    unittest.main()
