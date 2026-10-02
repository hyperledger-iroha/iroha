# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Reference software renderer and a platform-neutral draw list.

The picture is black, with four blossom finders in the canvas corners and a
``天``-shaped field of 256 tiles inside three dotted rings. A light tile is a
pale rounded square with a near-black katakana; a dark tile is empty except
for its sakura-pink katakana.

:func:`render_frame` reproduces the Rust reference renderer pixel for pixel:
every supersample is shaded by the same predicates in the same IEEE double
arithmetic. It only evaluates samples near something that is drawn (finders,
tile squares and lit dots), which is equivalent because every other sample is
background.

:func:`draw_list` describes the same frame as shapes for vector backends
(canvas, SVG, Core Graphics, ...). Platform renderers may use their own 2D
APIs as long as geometry and polarity match the reference.
"""

from __future__ import annotations

import functools
import math
from bisect import bisect_left, bisect_right
from dataclasses import dataclass, field
from itertools import repeat
from operator import add, floordiv
from typing import List, Optional, Sequence, Tuple

from ._numeric import round_half_away
from .glyphs import GLYPH_COUNT, GLYPH_GRID, STROKE_WIDTH, STROKES
from .image import Rgb
from .lanes import FrameCells
from .layout import (
    CANVAS,
    CENTER,
    DOT_RADIUS,
    FINDER_CENTERS,
    FINDER_CORE,
    FINDER_NOTCH_RADIUS,
    FINDER_OUTER,
    FINDER_PETAL_RADIUS,
    GLYPH_BOX,
    RING_COUNT,
    RING_RADII,
    RING_SLOTS,
    TILE_COUNT,
    TILE_ORIGIN,
    TILE_PITCH,
    TILE_SIZE,
    TILES,
    finder_lit,
    finder_notch_centers,
    finder_petal_centers,
    ring_offset,
    tile_center,
)

__all__ = [
    "Color",
    "Palette",
    "RenderOptions",
    "render_frame",
    "TILE_CORNER_RADIUS",
    "Circle",
    "FinderShape",
    "TileShape",
    "DrawList",
    "draw_list",
]

Color = Tuple[int, int, int]

#: Corner radius of a tile's rounded square, in design units.
TILE_CORNER_RADIUS = 3.0


@dataclass(frozen=True)
class Palette:
    """Colours of the picture."""

    #: Background.
    background: Color = (0, 0, 0)
    #: Light tile fill and finders.
    light: Color = (250, 235, 244)
    #: Sakura pink of dots and of glyphs on dark tiles.
    pink: Color = (245, 175, 208)
    #: Glyph colour on a light tile.
    ink: Color = (20, 4, 14)


@dataclass(frozen=True)
class RenderOptions:
    """Rendering options."""

    #: Output side in pixels.
    size: int = 1024
    #: Samples per pixel side for anti-aliasing (1-4).
    supersample: int = 3
    #: Colours.
    palette: Palette = field(default_factory=Palette)


# Sample colour codes painted before the colour lookup.
_BACKGROUND, _LIGHT, _PINK, _INK = 0, 1, 2, 3

_BITMAP_N = 128


@functools.lru_cache(maxsize=None)
def _glyph_bitmaps() -> Tuple[bytes, ...]:
    """128x128 ink bitmaps of every glyph over the glyph design grid.

    Pixel ``(u, v)`` is inked when its centre lies within half a stroke width
    of a stroke segment, exactly as the reference's ``is_inked``. Segments are
    tested only near their bounding box, which cannot change the result.
    """
    n = _BITMAP_N
    radius = STROKE_WIDTH / 2.0
    reach = radius + 0.25  # generous: anything farther is certainly not inked
    coords = [(i + 0.5) / n * GLYPH_GRID for i in range(n)]
    out = []
    for glyph in range(GLYPH_COUNT):
        bitmap = bytearray(n * n)
        for stroke in STROKES[glyph]:
            for (ax, ay), (bx, by) in zip(stroke, stroke[1:]):
                dx = bx - ax
                dy = by - ay
                length_sq = dx * dx + dy * dy
                u_lo = bisect_left(coords, min(ax, bx) - reach)
                u_hi = bisect_right(coords, max(ax, bx) + reach)
                v_lo = bisect_left(coords, min(ay, by) - reach)
                v_hi = bisect_right(coords, max(ay, by) + reach)
                for v in range(v_lo, v_hi):
                    y = coords[v]
                    base = v * n
                    for u in range(u_lo, u_hi):
                        if bitmap[base + u]:
                            continue
                        x = coords[u]
                        if length_sq == 0.0:
                            t = 0.0
                        else:
                            t = ((x - ax) * dx + (y - ay) * dy) / length_sq
                            if t < 0.0:
                                t = 0.0
                            elif t > 1.0:
                                t = 1.0
                        ex = x - (ax + t * dx)
                        ey = y - (ay + t * dy)
                        if math.sqrt(ex * ex + ey * ey) <= radius:
                            bitmap[base + u] = 1
        out.append(bytes(bitmap))
    return tuple(out)


def _ring_is_pink(dx: float, dy: float, dots: Sequence[bool]) -> bool:
    """The reference's ring-dot test for a sample at ``(dx, dy)`` from the centre."""
    radius = math.sqrt(dx * dx + dy * dy)
    for ring in range(RING_COUNT):
        ring_radius = RING_RADII[ring]
        if abs(radius - ring_radius) > DOT_RADIUS:
            continue
        slots = RING_SLOTS[ring]
        theta = math.atan2(dy, dx)
        if theta < 0.0:
            theta += math.tau
        slot = int(round_half_away(theta / math.tau * slots)) % slots
        if not dots[ring_offset(ring) + slot]:
            continue
        angle = math.tau * slot / slots
        ex = dx - ring_radius * math.cos(angle)
        ey = dy - ring_radius * math.sin(angle)
        if math.sqrt(ex * ex + ey * ey) <= DOT_RADIUS:
            return True
    return False


def _paint(cells: FrameCells, coords: Sequence[float], codes: bytearray) -> None:
    span = len(coords)
    # finders: the disc of radius FINDER_OUTER belongs to the blossom
    outer = FINDER_OUTER
    for fx, fy in FINDER_CENTERS:
        cols = range(bisect_left(coords, fx - outer - 1.0), bisect_right(coords, fx + outer + 1.0))
        for j in range(
            bisect_left(coords, fy - outer - 1.0), bisect_right(coords, fy + outer + 1.0)
        ):
            dy = coords[j] - fy
            dy2 = dy * dy
            base = j * span
            for i in cols:
                dx = coords[i] - fx
                if math.sqrt(dx * dx + dy2) <= outer and finder_lit(dx, dy):
                    codes[base + i] = _LIGHT
    # tiles: only the rounded square of a tile cell is ever drawn
    half = TILE_SIZE / 2.0
    straight = half - TILE_CORNER_RADIUS
    corner_sq = TILE_CORNER_RADIUS * TILE_CORNER_RADIUS
    box_half = GLYPH_BOX / 2.0
    box = 2.0 * box_half
    last = _BITMAP_N - 1

    def lattice_samples(index: int) -> List[Tuple[int, float, float, int]]:
        centre = TILE_ORIGIN + TILE_PITCH * (index + 0.5)
        out = []
        for i in range(bisect_left(coords, centre - half), bisect_right(coords, centre + half)):
            d = coords[i] - centre
            a = abs(d)
            if a > half:
                continue
            u = int((d + box_half) / box * _BITMAP_N) if a < box_half else -1
            out.append((i, d, a, min(u, last)))
        return out

    lattice = [lattice_samples(index) for index in range(20)]
    bitmaps = _glyph_bitmaps()
    for tile, (col, row) in enumerate(TILES):
        light = cells.light[tile]
        bitmap = bitmaps[cells.glyph[tile]]
        plain = _LIGHT if light else _BACKGROUND
        inked = _INK if light else _PINK
        columns = lattice[col]
        for j, _, ay, v in lattice[row]:
            base = j * span
            cy = ay - straight
            row_bits = v * _BITMAP_N if v >= 0 else -1
            for i, _, ax, u in columns:
                cx = ax - straight
                if not (cx <= 0.0 or cy <= 0.0 or cx * cx + cy * cy <= corner_sq):
                    continue
                if row_bits >= 0 and u >= 0 and bitmap[row_bits + u]:
                    codes[base + i] = inked
                elif plain:
                    codes[base + i] = plain
    # ring dots: paint around every lit slot
    dots = cells.dots
    reach = DOT_RADIUS + 1.0
    for ring in range(RING_COUNT):
        slots = RING_SLOTS[ring]
        ring_radius = RING_RADII[ring]
        offset = ring_offset(ring)
        for slot in range(slots):
            if not dots[offset + slot]:
                continue
            angle = math.tau * slot / slots
            px = CENTER + ring_radius * math.cos(angle)
            py = CENTER + ring_radius * math.sin(angle)
            cols = range(bisect_left(coords, px - reach), bisect_right(coords, px + reach))
            for j in range(bisect_left(coords, py - reach), bisect_right(coords, py + reach)):
                dy = coords[j] - CENTER
                base = j * span
                for i in cols:
                    if _ring_is_pink(coords[i] - CENTER, dy, dots):
                        codes[base + i] = _PINK


def render_frame(cells: FrameCells, options: Optional[RenderOptions] = None) -> Rgb:
    """Render one frame as RGB, identical to the reference renderer.

    Raises :class:`ValueError` when ``options.size`` is not positive or
    ``options.supersample`` is outside 1-4.
    """
    if options is None:
        options = RenderOptions()
    size = options.size
    s = options.supersample
    if size <= 0 or not 1 <= s <= 4:
        raise ValueError("render size must be positive and supersample within 1-4")
    unit = CANVAS / size
    span = size * s
    coords = [(i // s + ((i % s) + 0.5) / s) * unit for i in range(span)]
    codes = bytearray(span * span)
    _paint(cells, coords, codes)
    palette = options.palette
    colours = (palette.background, palette.light, palette.pink, palette.ink)
    tables = [bytes(c[k] for c in colours) + bytes(252) for k in range(3)]
    n = s * s
    half = n // 2
    out = bytearray(size * size * 3)
    stride = size * 3
    for py in range(size):
        sums: List[Optional[list]] = [None, None, None]
        for sy in range(s):
            start = (py * s + sy) * span
            line = codes[start : start + span]
            for k in range(3):
                values = line.translate(tables[k])
                acc = list(values[0::s])
                for sx in range(1, s):
                    acc = list(map(add, acc, values[sx::s]))
                previous = sums[k]
                sums[k] = acc if previous is None else list(map(add, previous, acc))
        row = py * stride
        for k in range(3):
            total = sums[k]
            assert total is not None
            out[row + k : row + stride : 3] = bytes(
                map(floordiv, map(add, total, repeat(half)), repeat(n))
            )
    return Rgb(size, size, out)


@dataclass(frozen=True)
class Circle:
    """A filled circle in canvas units."""

    x: float
    y: float
    radius: float


@dataclass(frozen=True)
class FinderShape:
    """A blossom finder: fill ``core`` and ``petals`` light, then ``notches`` background."""

    #: Finder centre.
    center: Tuple[float, float]
    #: Solid centre disc.
    core: Circle
    #: Five petal circles, the first pointing up.
    petals: Tuple[Circle, ...]
    #: Five notch circles cut into the petal tips.
    notches: Tuple[Circle, ...]


@dataclass(frozen=True)
class TileShape:
    """One data tile and its katakana."""

    #: Tile index (``0..256``) in row-major mask order.
    index: int
    #: Left edge of the rounded square.
    x: float
    #: Top edge of the rounded square.
    y: float
    #: Side of the rounded square (25 units).
    size: float
    #: Corner radius of the rounded square (3 units).
    corner_radius: float
    #: Whether the tile is light (filled) or dark (background).
    light: bool
    #: Glyph symbol ``0..16`` (see :data:`~iroha_petal.glyphs.GLYPH_CHARS`).
    glyph: int
    #: Fill of the rounded square, ``None`` for a dark tile.
    fill: Optional[Color]
    #: Colour of the glyph strokes (ink on light tiles, pink on dark tiles).
    glyph_color: Color
    #: Glyph box ``(x, y, side)``; strokes are clipped to it.
    glyph_box: Tuple[float, float, float]
    #: Stroke polylines in canvas units, drawn with round caps and joins.
    strokes: Tuple[Tuple[Tuple[float, float], ...], ...]
    #: Stroke width in canvas units (6.5/32 of the glyph box).
    stroke_width: float


@dataclass(frozen=True)
class DrawList:
    """Shapes of one frame in canvas units (``0..1024``), in painting order."""

    #: Canvas side in design units.
    canvas: float
    #: Colours.
    palette: Palette
    #: The four corner finders, clockwise from the top-left one.
    finders: Tuple[FinderShape, ...]
    #: Every tile in index order.
    tiles: Tuple[TileShape, ...]
    #: Every lit ring dot (gates included), filled pink.
    dots: Tuple[Circle, ...]

    def to_svg(self, size: Optional[int] = None) -> str:
        """Serialise the draw list as a standalone SVG document."""
        side = self.canvas if size is None else size
        p = self.palette

        def rgb(color: Color) -> str:
            return "#%02x%02x%02x" % tuple(color)

        def num(value: float) -> str:
            text = ("%.4f" % value).rstrip("0").rstrip(".")
            return text if text not in ("", "-0") else "0"

        def circle(c: Circle) -> str:
            return f'<circle cx="{num(c.x)}" cy="{num(c.y)}" r="{num(c.radius)}"/>'

        parts = [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{num(side)}" height="{num(side)}" '
            f'viewBox="0 0 {num(self.canvas)} {num(self.canvas)}">',
            f'<rect width="{num(self.canvas)}" height="{num(self.canvas)}" fill="{rgb(p.background)}"/>',
            f'<g fill="{rgb(p.light)}">',
        ]
        for finder in self.finders:
            parts.append(circle(finder.core))
            parts.extend(circle(c) for c in finder.petals)
        parts.append(f'</g><g fill="{rgb(p.background)}">')
        for finder in self.finders:
            parts.extend(circle(c) for c in finder.notches)
        parts.append("</g>")
        for tile in self.tiles:
            if tile.fill is not None:
                parts.append(
                    f'<rect x="{num(tile.x)}" y="{num(tile.y)}" width="{num(tile.size)}" '
                    f'height="{num(tile.size)}" rx="{num(tile.corner_radius)}" fill="{rgb(tile.fill)}"/>'
                )
            gx, gy, gs = tile.glyph_box
            parts.append(
                f'<svg x="{num(gx)}" y="{num(gy)}" width="{num(gs)}" height="{num(gs)}" '
                f'viewBox="{num(gx)} {num(gy)} {num(gs)} {num(gs)}" overflow="hidden">'
                f'<g fill="none" stroke="{rgb(tile.glyph_color)}" stroke-width="{num(tile.stroke_width)}" '
                f'stroke-linecap="round" stroke-linejoin="round">'
            )
            for stroke in tile.strokes:
                points = " ".join(f"{num(x)},{num(y)}" for x, y in stroke)
                parts.append(f'<polyline points="{points}"/>')
            parts.append("</g></svg>")
        parts.append(f'<g fill="{rgb(p.pink)}">')
        parts.extend(circle(c) for c in self.dots)
        parts.append("</g></svg>")
        return "\n".join(parts) + "\n"


def draw_list(cells: FrameCells, palette: Optional[Palette] = None) -> DrawList:
    """Describe ``cells`` as shapes for a vector backend."""
    if palette is None:
        palette = Palette()
    finders = []
    for fx, fy in FINDER_CENTERS:
        finders.append(
            FinderShape(
                center=(fx, fy),
                core=Circle(fx, fy, FINDER_CORE),
                petals=tuple(
                    Circle(fx + px, fy + py, FINDER_PETAL_RADIUS)
                    for px, py in finder_petal_centers()
                ),
                notches=tuple(
                    Circle(fx + nx, fy + ny, FINDER_NOTCH_RADIUS)
                    for nx, ny in finder_notch_centers()
                ),
            )
        )
    half = TILE_SIZE / 2.0
    box_half = GLYPH_BOX / 2.0
    scale = GLYPH_BOX / GLYPH_GRID
    tiles = []
    for index in range(TILE_COUNT):
        cx, cy = tile_center(index)
        light = cells.light[index]
        glyph = cells.glyph[index]
        gx = cx - box_half
        gy = cy - box_half
        tiles.append(
            TileShape(
                index=index,
                x=cx - half,
                y=cy - half,
                size=TILE_SIZE,
                corner_radius=TILE_CORNER_RADIUS,
                light=light,
                glyph=glyph,
                fill=palette.light if light else None,
                glyph_color=palette.ink if light else palette.pink,
                glyph_box=(gx, gy, GLYPH_BOX),
                strokes=tuple(
                    tuple((gx + x * scale, gy + y * scale) for x, y in stroke)
                    for stroke in STROKES[glyph]
                ),
                stroke_width=STROKE_WIDTH * scale,
            )
        )
    dots = []
    for ring in range(RING_COUNT):
        slots = RING_SLOTS[ring]
        offset = ring_offset(ring)
        for slot in range(slots):
            if cells.dots[offset + slot]:
                angle = math.tau * slot / slots
                dots.append(
                    Circle(
                        CENTER + RING_RADII[ring] * math.cos(angle),
                        CENTER + RING_RADII[ring] * math.sin(angle),
                        DOT_RADIUS,
                    )
                )
    return DrawList(
        canvas=CANVAS,
        palette=palette,
        finders=tuple(finders),
        tiles=tuple(tiles),
        dots=tuple(dots),
    )
