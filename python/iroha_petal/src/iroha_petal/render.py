# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Reference software renderer and a platform-neutral draw list.

The picture is black, with four sakura-blossom finders in the canvas corners,
three dotted rings, and inside them a field of petals on concentric bands, each
lit petal turned about its centre by its turn. A lit petal whose centre lies
inside the ``天`` is drawn in the light colour, the others in sakura pink, over a
faint backdrop of the ``天`` itself, so the character shows through the storm of
petals.

:func:`render_frame` reproduces the Rust reference renderer pixel for pixel:
every supersample is shaded by the same predicates in the same IEEE double
arithmetic. It paints each drawn shape only over the samples near it (the
backdrop row by row, every lit petal over its own ellipse, every lit dot and
finder over its disc), which is equivalent because the finders, the rings and
the field never overlap; a sample that two petals claim, or that sits at a
petal tip, is shaded by the reference's own petal search.

:func:`draw_list` describes the same frame as shapes for vector backends
(canvas, SVG, Core Graphics, ...). Platform renderers may use their own 2D
APIs as long as geometry, turns and polarity match the reference.
"""

from __future__ import annotations

import math
from bisect import bisect_left, bisect_right
from dataclasses import dataclass, field
from itertools import repeat
from operator import add, floordiv
from typing import List, Optional, Sequence, Set, Tuple

from ._numeric import round_half_away
from .image import Rgb
from .lanes import FrameCells
from .layout import (
    BACKDROP_RADIUS,
    BAND_COUNT,
    BAND_INNER,
    BAND_OFFSETS,
    BAND_SLOTS,
    BAND_SPACING,
    CANVAS,
    CENTER,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    FINDER_CORE,
    FINDER_NOTCH_RADIUS,
    FINDER_OUTER,
    FINDER_PETAL_RADIUS,
    MASK,
    MASK_GRID,
    MASK_ORIGIN,
    MASK_PITCH,
    PETAL_HALF_LENGTH,
    PETAL_HALF_WIDTH,
    PETAL_NOTCH_DISTANCE,
    PETAL_NOTCH_RADIUS,
    RING_COUNT,
    RING_RADII,
    RING_SLOTS,
    band_phase,
    band_radius,
    cell_angle,
    cell_center,
    cell_light,
    finder_lit,
    finder_notch_centers,
    finder_petal_centers,
    in_mask,
    petal_lit,
    ring_offset,
)
from .turns import TURN_POSITIONS, turn_angle

__all__ = [
    "Color",
    "Palette",
    "RenderOptions",
    "render_frame",
    "Circle",
    "FinderShape",
    "BackdropCell",
    "PetalShape",
    "DrawList",
    "draw_list",
]

Color = Tuple[int, int, int]


@dataclass(frozen=True)
class Palette:
    """Colours of the picture."""

    #: Background.
    background: Color = (0, 0, 0)
    #: Finders and the petals inside the ``天``.
    light: Color = (250, 235, 244)
    #: Sakura pink of the ring dots and of the petals outside the ``天``.
    pink: Color = (245, 175, 208)
    #: The faint ``天`` drawn under the petals (a tenth of the light colour).
    backdrop: Color = (25, 24, 24)


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
_BACKGROUND, _LIGHT, _PINK, _BACKDROP = 0, 1, 2, 3

#: The axis of every field cell's petal at every turn position (the outward direction plus
#: the turn), ``[cell * 4 + turn]``.
_AXIS = tuple(
    cell_angle(cell) + turn_angle(turn)
    for cell in range(FIELD_CELLS)
    for turn in range(TURN_POSITIONS)
)
_AXIS_COS = tuple(math.cos(angle) for angle in _AXIS)
_AXIS_SIN = tuple(math.sin(angle) for angle in _AXIS)
_CELL_CODE = tuple(_LIGHT if cell_light(cell) else _PINK for cell in range(FIELD_CELLS))
_NOTCH_RADIUS_SQUARED = PETAL_NOTCH_RADIUS * PETAL_NOTCH_RADIUS
#: Samples this close to a petal tip are shaded by the reference's petal search: only
#: there can its band test (``|radius - band radius| <= half-length``) disagree with the
#: petal's own ellipse, however the petal is turned (a point of the ellipse within this
#: distance of the tips along its axis lies less than the half-length from its centre).
_TIP = PETAL_HALF_LENGTH - 0.01


def _round(value: float) -> float:
    """Rust's ``f64::round`` (halves away from zero) for finite values."""
    if value < 0.0:
        return -round_half_away(-value)
    return round_half_away(value)


def _petal_code(cells: FrameCells, x: float, y: float) -> Optional[int]:
    """The reference's petal search for the canvas sample ``(x, y)``: the colour code of
    the first lit petal that covers it, turned by its turn, or ``None``."""
    petals = cells.petals
    turns = cells.turns
    dx = x - CENTER
    dy = y - CENTER
    radius = math.sqrt(dx * dx + dy * dy)
    theta = math.atan2(dy, dx)
    if theta < 0.0:
        theta += math.tau
    reach = PETAL_HALF_LENGTH
    nearest = _round((radius - BAND_INNER) / BAND_SPACING)
    for band in (nearest - 1.0, nearest, nearest + 1.0):
        if band < 0.0 or band >= BAND_COUNT:
            continue
        b = int(band)
        if abs(radius - band_radius(b)) > reach:
            continue
        slots = BAND_SLOTS[b]
        around = int(_round(theta / math.tau * slots - band_phase(b)))
        for slot in (around - 1, around, around + 1):
            cell = BAND_OFFSETS[b] + slot % slots
            if not petals[cell]:
                continue
            cx, cy = cell_center(cell)
            if petal_lit(x - cx, y - cy, cell_angle(cell) + turn_angle(turns[cell])):
                return _CELL_CODE[cell]
    return None


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


def _paint_backdrop(coords: Sequence[float], codes: bytearray) -> None:
    """The ``#`` lattice cells inside :data:`BACKDROP_RADIUS`."""
    span = len(coords)
    offsets = [x - CENTER for x in coords]
    # the lattice column of every sample column (`in_mask`), as one pattern per mask row
    columns = []
    for x in coords:
        col = int((x - MASK_ORIGIN) / MASK_PITCH) if x >= MASK_ORIGIN else MASK_GRID
        columns.append(col if col < MASK_GRID else MASK_GRID)
    patterns = [
        bytes(_BACKDROP if col < MASK_GRID and line[col] == "#" else _BACKGROUND for col in columns)
        for line in MASK
    ]
    middle = bisect_left(offsets, 0.0)

    def inside(i: int, dy2: float) -> bool:
        dx = offsets[i]
        return math.sqrt(dx * dx + dy2) <= BACKDROP_RADIUS

    for j, y in enumerate(coords):
        if y < MASK_ORIGIN:
            continue
        row = int((y - MASK_ORIGIN) / MASK_PITCH)
        if row >= MASK_GRID:
            continue
        dy = y - CENTER
        dy2 = dy * dy
        # the samples within the radius form one run around the column nearest the centre
        if middle < span and inside(middle, dy2):
            m = middle
        elif middle > 0 and inside(middle - 1, dy2):
            m = middle - 1
        else:
            continue
        lo, hi = 0, m
        while lo < hi:
            mid = (lo + hi) // 2
            if inside(mid, dy2):
                hi = mid
            else:
                lo = mid + 1
        first = lo
        lo, hi = m, span - 1
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if inside(mid, dy2):
                lo = mid
            else:
                hi = mid - 1
        last = lo
        base = j * span
        codes[base + first : base + last + 1] = patterns[row][first : last + 1]


def _paint_petals(cells: FrameCells, coords: Sequence[float], codes: bytearray) -> None:
    """Every lit petal over its ellipse along its turned axis; contested and tip samples
    are shaded by the reference's petal search."""
    span = len(coords)
    petals = cells.petals
    turns = cells.turns
    a = PETAL_HALF_LENGTH
    b = PETAL_HALF_WIDTH
    a2 = a * a
    b2 = b * b
    notch = PETAL_NOTCH_DISTANCE
    special: Set[int] = set()
    for cell in range(FIELD_CELLS):
        if not petals[cell]:
            continue
        cx, cy = cell_center(cell)
        axis = cell * TURN_POSITIONS + turns[cell]
        cos = _AXIS_COS[axis]
        sin = _AXIS_SIN[axis]
        code = _CELL_CODE[cell]
        # the ellipse's quadratic in dx for a row at dy: qa dx^2 + qb dx + qc <= 0; the
        # bounds only choose the samples to test, with a margin
        qa = cos * cos / a2 + sin * sin / b2
        cross = cos * sin * (1.0 / a2 - 1.0 / b2)
        tall = math.sqrt(a2 * sin * sin + b2 * cos * cos) + 0.05
        for j in range(bisect_left(coords, cy - tall), bisect_right(coords, cy + tall)):
            dy = coords[j] - cy
            qb = 2.0 * dy * cross
            qc = dy * dy * (sin * sin / a2 + cos * cos / b2) - 1.0
            disc = qb * qb - 4.0 * qa * qc
            half = math.sqrt(disc) / (2.0 * qa) if disc > 0.0 else 0.0
            middle = -qb / (2.0 * qa)
            base = j * span
            for i in range(
                bisect_left(coords, cx + middle - half - 0.05),
                bisect_right(coords, cx + middle + half + 0.05),
            ):
                dx = coords[i] - cx
                along = dx * cos + dy * sin
                across = -dx * sin + dy * cos
                u = along / a
                v = across / b
                if u * u + v * v > 1.0:
                    continue
                tip = along - notch
                if not tip * tip + across * across > _NOTCH_RADIUS_SQUARED:
                    continue
                index = base + i
                if along > _TIP or along < -_TIP:
                    special.add(index)
                    continue
                current = codes[index]
                if current == _LIGHT or current == _PINK:
                    special.add(index)
                else:
                    codes[index] = code
    for index in special:
        j, i = divmod(index, span)
        x = coords[i]
        y = coords[j]
        resolved = _petal_code(cells, x, y)
        if resolved is None:
            dx = x - CENTER
            dy = y - CENTER
            inside = math.sqrt(dx * dx + dy * dy) <= BACKDROP_RADIUS
            resolved = _BACKDROP if inside and in_mask(x, y) else _BACKGROUND
        codes[index] = resolved


def _paint(cells: FrameCells, coords: Sequence[float], codes: bytearray) -> None:
    # The finders, the ring dots and the field (petals over the backdrop) occupy disjoint
    # parts of the canvas, so painting each over its own samples shades every sample as
    # the reference's priority order (finders, dots, petals, backdrop) does.
    span = len(coords)
    _paint_backdrop(coords, codes)
    _paint_petals(cells, coords, codes)
    # ring dots: paint around every lit slot. Near a slot only that slot's dot can be drawn
    # (slots are 29 units apart, rings 35), and a sample well inside its disc passes every
    # test of the reference's ring search, so only the rim needs the search itself.
    dots = cells.dots
    reach = DOT_RADIUS + 1.0
    inner = DOT_RADIUS - 1e-6
    offsets = [x - CENTER for x in coords]
    for ring in range(RING_COUNT):
        slots = RING_SLOTS[ring]
        ring_radius = RING_RADII[ring]
        offset = ring_offset(ring)
        for slot in range(slots):
            if not dots[offset + slot]:
                continue
            angle = math.tau * slot / slots
            px = ring_radius * math.cos(angle)
            py = ring_radius * math.sin(angle)
            cols = range(
                bisect_left(coords, CENTER + px - reach), bisect_right(coords, CENTER + px + reach)
            )
            for j in range(
                bisect_left(coords, CENTER + py - reach), bisect_right(coords, CENTER + py + reach)
            ):
                dy = offsets[j]
                ey = dy - py
                ey2 = ey * ey
                base = j * span
                for i in cols:
                    dx = offsets[i]
                    ex = dx - px
                    distance = math.sqrt(ex * ex + ey2)
                    if distance > DOT_RADIUS:
                        continue
                    if distance <= inner or _ring_is_pink(dx, dy, dots):
                        codes[base + i] = _PINK
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
                if math.sqrt(dx * dx + dy2) <= outer:
                    codes[base + i] = _LIGHT if finder_lit(dx, dy) else _BACKGROUND


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
    colours = (palette.background, palette.light, palette.pink, palette.backdrop)
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
class BackdropCell:
    """One ``#`` cell of the ``天`` mask lattice, a square filled with the backdrop colour
    and clipped to :attr:`DrawList.backdrop_clip`."""

    #: Left edge.
    x: float
    #: Top edge.
    y: float
    #: Side (the 33-unit lattice pitch).
    size: float


@dataclass(frozen=True)
class PetalShape:
    """One lit petal: an ellipse of :attr:`DrawList.petal_half_length` along ``angle``
    (its axis: the outward direction plus its turn) and :attr:`DrawList.petal_half_width`
    across it, centred on the field cell, minus the notch at its tip."""

    #: Field cell (``0..3376``).
    index: int
    #: Centre ``x``.
    x: float
    #: Centre ``y``.
    y: float
    #: The petal's axis in radians, clockwise on screen from 3 o'clock: the cell's outward
    #: direction plus its turn (see :func:`~iroha_petal.turns.turn_angle`).
    angle: float
    #: Whether the petal lies inside the ``天`` (light) or not (pink).
    light: bool
    #: Fill colour.
    color: Color


@dataclass(frozen=True)
class DrawList:
    """Shapes of one frame in canvas units (``0..1024``), in painting order: background,
    then the backdrop cells clipped to :attr:`backdrop_clip`, the lit petals, the lit
    dots and the finders.

    Every petal is an ellipse along its axis minus its notch (a disc of
    :attr:`petal_notch_radius` centred :attr:`petal_notch_distance` along the axis), which
    must be drawn: it tells a petal from one turned by half a turn, and the turn reader's
    templates include it. Petals are small (12 x 7.5 units): draw them antialiased, at 768
    pixels or more.
    """

    #: Canvas side in design units.
    canvas: float
    #: Colours.
    palette: Palette
    #: The ``#`` cells of the mask lattice, row-major.
    backdrop: Tuple[BackdropCell, ...]
    #: The disc the backdrop is clipped to (radius 390 about the canvas centre).
    backdrop_clip: Circle
    #: Every lit petal in field-cell order.
    petals: Tuple[PetalShape, ...]
    #: Half the length of a petal, along its axis.
    petal_half_length: float
    #: Half the width of a petal.
    petal_half_width: float
    #: Radius of the notch cut into a petal tip (always drawn).
    petal_notch_radius: float
    #: Distance from a petal centre to its notch centre, along its axis.
    petal_notch_distance: float
    #: Every lit ring dot (gates included), filled pink.
    dots: Tuple[Circle, ...]
    #: The four corner finders, clockwise from the top-left one.
    finders: Tuple[FinderShape, ...]

    def petal_outline(self) -> str:
        """The SVG path data of the petal outline in its own frame (``x`` along its axis,
        the centre at the origin): the ellipse the long way round from one edge of the
        notch to the other, then the notch's inner arc back."""
        a = self.petal_half_length
        b = self.petal_half_width
        r = self.petal_notch_radius
        d = self.petal_notch_distance
        # (a cos t - d)^2 + (b sin t)^2 = r^2 is a quadratic in cos t
        qa = a * a - b * b
        qb = -2.0 * a * d
        qc = d * d + b * b - r * r
        cos = None
        discriminant = qb * qb - 4.0 * qa * qc
        if qa != 0.0 and discriminant >= 0.0:
            root = math.sqrt(discriminant)
            cos = next(
                (
                    c
                    for c in ((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa))
                    if -1.0 < c < 1.0
                ),
                None,
            )
        if r <= 0.0 or cos is None:
            # the notch does not cut the outline
            return (
                f"M {_svg_number(a)} 0 A {_svg_number(a)} {_svg_number(b)} 0 1 1 "
                f"{_svg_number(-a)} 0 A {_svg_number(a)} {_svg_number(b)} 0 1 1 {_svg_number(a)} 0 Z"
            )
        x = _svg_number(a * cos)
        y = _svg_number(b * math.sqrt(1.0 - cos * cos))
        return (
            f"M {x} {y} A {_svg_number(a)} {_svg_number(b)} 0 1 1 {x} -{y} "
            f"A {_svg_number(r)} {_svg_number(r)} 0 0 0 {x} {y} Z"
        )

    def to_svg(self, size: Optional[int] = None) -> str:
        """Serialise the draw list as a standalone SVG document; every petal is drawn
        notched along its turned axis, and SVG renderers antialias it."""
        side = self.canvas if size is None else size
        p = self.palette

        def rgb(color: Color) -> str:
            return "#%02x%02x%02x" % tuple(color)

        num = _svg_number

        def circle(c: Circle) -> str:
            return f'<circle cx="{num(c.x)}" cy="{num(c.y)}" r="{num(c.radius)}"/>'

        clip = self.backdrop_clip
        parts = [
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{num(side)}" height="{num(side)}" '
            f'viewBox="0 0 {num(self.canvas)} {num(self.canvas)}">',
            f'<rect width="{num(self.canvas)}" height="{num(self.canvas)}" fill="{rgb(p.background)}"/>',
            f'<defs><clipPath id="backdrop">{circle(clip)}</clipPath>'
            f'<path id="petal" d="{self.petal_outline()}"/></defs>',
            f'<g fill="{rgb(p.backdrop)}" clip-path="url(#backdrop)">',
        ]
        for cell in self.backdrop:
            parts.append(
                f'<rect x="{num(cell.x)}" y="{num(cell.y)}" width="{num(cell.size)}" '
                f'height="{num(cell.size)}"/>'
            )
        parts.append("</g>")
        for light in (True, False):
            parts.append(f'<g fill="{rgb(p.light if light else p.pink)}">')
            for petal in self.petals:
                if petal.light is light:
                    parts.append(
                        f'<use href="#petal" transform="translate({num(petal.x)} {num(petal.y)}) '
                        f'rotate({num(math.degrees(petal.angle))})"/>'
                    )
            parts.append("</g>")
        parts.append(f'<g fill="{rgb(p.pink)}">')
        parts.extend(circle(c) for c in self.dots)
        parts.append(f'</g><g fill="{rgb(p.light)}">')
        for finder in self.finders:
            parts.append(circle(finder.core))
            parts.extend(circle(c) for c in finder.petals)
        parts.append(f'</g><g fill="{rgb(p.background)}">')
        for finder in self.finders:
            parts.extend(circle(c) for c in finder.notches)
        parts.append("</g></svg>")
        return "\n".join(parts) + "\n"


def _svg_number(value: float) -> str:
    """``value`` with at most four decimals, for SVG attributes."""
    text = ("%.4f" % value).rstrip("0").rstrip(".")
    return text if text not in ("", "-0") else "0"


_BACKDROP_CELLS = tuple(
    BackdropCell(MASK_ORIGIN + MASK_PITCH * col, MASK_ORIGIN + MASK_PITCH * row, MASK_PITCH)
    for row, line in enumerate(MASK)
    for col, mark in enumerate(line)
    if mark == "#"
)


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
    petals = []
    for cell in range(FIELD_CELLS):
        if cells.petals[cell]:
            x, y = cell_center(cell)
            light = cell_light(cell)
            petals.append(
                PetalShape(
                    index=cell,
                    x=x,
                    y=y,
                    angle=cell_angle(cell) + turn_angle(cells.turns[cell]),
                    light=light,
                    color=palette.light if light else palette.pink,
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
        backdrop=_BACKDROP_CELLS,
        backdrop_clip=Circle(CENTER, CENTER, BACKDROP_RADIUS),
        petals=tuple(petals),
        petal_half_length=PETAL_HALF_LENGTH,
        petal_half_width=PETAL_HALF_WIDTH,
        petal_notch_radius=PETAL_NOTCH_RADIUS,
        petal_notch_distance=PETAL_NOTCH_DISTANCE,
        dots=tuple(dots),
        finders=tuple(finders),
    )
