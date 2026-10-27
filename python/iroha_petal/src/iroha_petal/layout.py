# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Normative frame geometry.

All coordinates are *design units* on a square canvas of :data:`CANVAS` units
with the origin at the top-left and ``y`` growing downward. A renderer scales
the canvas to any pixel size; a decoder maps camera pixels back to design
units. Nothing here depends on floating-point rounding for bit placement: cell
*order* is defined by integer indices only, and the slot count of every band is
a fixed table.

The Rust reference stores the layout constants as ``f32`` and widens them to
``f64`` where it computes. Most of them are exact small integers; the petal
constants (``4.6``, ``3.73``, ...) are not, so they hold here the widened
single-precision values the reference computes with (``PETAL_RADIUS`` is
``4.599999904632568``). :func:`slot_center` evaluates its trigonometry in single
precision like the reference.

The frame is a *card*: the square with a band of katakana tiles above and below
it (:data:`CARD_HEIGHT`). Coordinates stay the square's: the band above the
square has negative ``y``, the band below it ``y`` beyond :data:`CANVAS`, and a
card renderer moves everything down by :data:`SQUARE_TOP`.
"""

from __future__ import annotations

import enum
import math
import struct
from bisect import bisect_right
from dataclasses import dataclass
from typing import List, Optional, Tuple

from .glyphs import GLYPH_GRID, is_inked

__all__ = [
    "CANVAS",
    "CENTER",
    "BAND_COUNT",
    "BAND_INNER",
    "BAND_SPACING",
    "BAND_SLOTS",
    "BAND_OFFSETS",
    "FIELD_CELLS",
    "SECTORS",
    "SECTOR_CELLS",
    "PETAL_RADIUS",
    "PETAL_HALF_LENGTH",
    "PETAL_HALF_WIDTH",
    "PETAL_NOTCH_RADIUS",
    "PETAL_NOTCH_DISTANCE",
    "band_radius",
    "band_phase",
    "cell_band_slot",
    "cell_angle",
    "cell_center",
    "sector_cells",
    "MASK_GRID",
    "MASK_ORIGIN",
    "MASK_PITCH",
    "BACKDROP_RADIUS",
    "MASK",
    "in_mask",
    "cell_light",
    "RING_COUNT",
    "RING_RADII",
    "RING_SLOTS",
    "DOT_RADIUS",
    "TOTAL_SLOTS",
    "GATE_DOTS",
    "D_BITS",
    "SlotKind",
    "SlotRole",
    "ring_offset",
    "slot_roles",
    "data_slots",
    "gate_slots",
    "guard_slots",
    "split_slot",
    "slot_center",
    "FINDER_CENTERS",
    "FINDER_CORE",
    "FINDER_PETALS",
    "FINDER_PETAL_DISTANCE",
    "FINDER_PETAL_RADIUS",
    "FINDER_NOTCH_RADIUS",
    "FINDER_OUTER",
    "finder_petal_centers",
    "finder_notch_centers",
    "finder_lit",
    "petal_lit",
    "KANA_TILE",
    "KANA_PITCH",
    "KANA_GLYPH_BOX",
    "KANA_CORNER",
    "KANA_ROWS",
    "KANA_COLUMNS",
    "KANA_GAP",
    "KANA_MARGIN",
    "KANA_BAND_HEIGHT",
    "CARD_HEIGHT",
    "SQUARE_TOP",
    "KANA_FINDER_CLEARANCE",
    "KANA_BANDS",
    "KANA_PILOTS",
    "KANA_PILOT_CELLS",
    "KANA_PILOT_SYMBOLS",
    "KanaTile",
    "kana_tiles",
    "kana_data_tiles",
    "kana_pilot_tiles",
    "kana_tile_at",
    "kana_inside",
    "kana_inked",
]

_F32 = struct.Struct("<f")


def _f32(value: float) -> float:
    """Round a double to the nearest IEEE single-precision value."""
    return _F32.unpack(_F32.pack(value))[0]


#: Canvas side length in design units.
CANVAS = 1024.0
#: Canvas centre coordinate on both axes.
CENTER = 512.0

#: Number of petal bands (concentric rings of field cells).
BAND_COUNT = 34
#: Radius of band 0.
BAND_INNER = 16.0
#: Radial distance between consecutive bands.
BAND_SPACING = 11.0
#: Cells on each band, inner band first: ``4 * round(2 pi r / 50)`` for the band
#: radius ``r``, about one cell every 12.5 units. Every count is a multiple of
#: four, so the four sectors split each band evenly and the field maps onto
#: itself under quarter turns. The table, not the formula, is normative.
BAND_SLOTS = (
    8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124,
    128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
)  # fmt: skip
#: Number of field cells.
FIELD_CELLS = 3376
#: Angular sectors of the field; each is one lane codeword.
SECTORS = 4
#: Cells in each sector.
SECTOR_CELLS = FIELD_CELLS // SECTORS
#: Radius of the disc a petal stands in for: decoders sample within it, and no other
#: petal, however it is turned, reaches into it (``4.6`` in single precision).
PETAL_RADIUS = _f32(4.6)
#: Half the length of a drawn petal, along its axis (the outward radial direction, turned
#: by the petal's turn, see :mod:`iroha_petal.turns`).
PETAL_HALF_LENGTH = _f32(6.0)
#: Half the width of a drawn petal (``3.73`` in single precision).
PETAL_HALF_WIDTH = _f32(3.73)
#: Radius of the notch cut into each petal tip; it tells a petal from one turned by half a
#: turn (``1.12`` in single precision).
PETAL_NOTCH_RADIUS = _f32(1.12)
#: Distance from a petal centre to its notch centre, along its axis (``6.24`` in single
#: precision).
PETAL_NOTCH_DISTANCE = _f32(6.24)


def _band_offsets() -> Tuple[int, ...]:
    offsets = [0]
    for slots in BAND_SLOTS:
        if slots % 4 != 0:
            raise AssertionError("band slot counts are multiples of four")
        offsets.append(offsets[-1] + slots)
    if offsets[-1] != FIELD_CELLS:
        raise AssertionError("the band table must hold every cell")
    return tuple(offsets)


#: Index of the first cell of every band (and the total at the end).
BAND_OFFSETS = _band_offsets()


def band_radius(band: int) -> float:
    """Radius of band ``band``."""
    return BAND_INNER + BAND_SPACING * band


def band_phase(band: int) -> float:
    """Fraction of a slot by which band ``band`` is turned: odd bands sit half a slot
    over, so neighbouring bands interleave."""
    return 0.5 if band % 2 == 1 else 0.0


def _check_cell(cell: int) -> int:
    if not 0 <= cell < FIELD_CELLS:
        raise IndexError("field cell out of range")
    return cell


def cell_band_slot(cell: int) -> Tuple[int, int]:
    """Band and slot of field cell ``cell``."""
    band = bisect_right(BAND_OFFSETS, _check_cell(cell)) - 1
    return band, cell - BAND_OFFSETS[band]


def _build_cells() -> Tuple[Tuple[float, ...], Tuple[float, ...], Tuple[float, ...]]:
    angles: List[float] = []
    xs: List[float] = []
    ys: List[float] = []
    for band, slots in enumerate(BAND_SLOTS):
        radius = band_radius(band)
        phase = band_phase(band)
        for slot in range(slots):
            angle = math.tau * (slot + phase) / slots
            angles.append(angle)
            xs.append(CENTER + radius * math.cos(angle))
            ys.append(CENTER + radius * math.sin(angle))
    return tuple(angles), tuple(xs), tuple(ys)


_CELL_ANGLES, _CELL_X, _CELL_Y = _build_cells()


def cell_angle(cell: int) -> float:
    """Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell
    ``cell``."""
    return _CELL_ANGLES[_check_cell(cell)]


def cell_center(cell: int) -> Tuple[float, float]:
    """Canvas coordinates of the centre of field cell ``cell`` (double precision)."""
    _check_cell(cell)
    return _CELL_X[cell], _CELL_Y[cell]


def _build_sectors() -> Tuple[Tuple[int, ...], ...]:
    sectors = []
    for sector in range(SECTORS):
        cells = []
        for band, slots in enumerate(BAND_SLOTS):
            quarter = slots // SECTORS
            for slot in range(sector * quarter, (sector + 1) * quarter):
                cells.append(BAND_OFFSETS[band] + slot)
        sectors.append(tuple(cells))
    return tuple(sectors)


_SECTOR_CELLS = _build_sectors()


def sector_cells(sector: int) -> Tuple[int, ...]:
    """The cells of sector ``sector`` in bit order: band by band from the centre out, and
    within a band in ascending slot order.

    Sector ``s`` holds slots ``s*n/4 ... (s+1)*n/4 - 1`` of every band of ``n`` slots.
    The first ``8 * FIELD_WORD`` (840) cells carry the sector's codeword; the last four
    are spare and stay dark.
    """
    if not 0 <= sector < SECTORS:
        raise IndexError("sector out of range")
    return _SECTOR_CELLS[sector]


#: Number of lattice columns and rows of the ``天`` mask.
MASK_GRID = 20
#: Canvas coordinate of the mask lattice's left and top edge.
MASK_ORIGIN = 182.0
#: Mask lattice pitch in design units.
MASK_PITCH = 33.0
#: Radius beyond which the ``天`` backdrop is not drawn.
BACKDROP_RADIUS = 390.0

#: The ``天`` silhouette, one string per lattice row from top to bottom. ``#``
#: marks a lattice cell inside the character. Petals whose centre lies in a
#: ``#`` cell are drawn in the light colour, the others in pink, and the ``#``
#: cells inside :data:`BACKDROP_RADIUS` are drawn as a faint backdrop, so the
#: character shows through the petals. The mask is mirror-symmetric left to
#: right and not top to bottom.
MASK = (
    "....############....",
    "...##############...",
    "..################..",
    ".##################.",
    "###..............###",
    "###..............###",
    "#########..#########",
    "#########..#########",
    "###..............###",
    "###..............###",
    "########....########",
    "########....########",
    "#######......#######",
    "#######..##..#######",
    "######..####..######",
    "#####...####...#####",
    ".###...######...###.",
    "..###.########.###..",
    "......########......",
    ".....##########.....",
)


def _lattice_index(offset: float) -> int:
    """Rust's ``(offset / MASK_PITCH) as usize`` for ``offset >= 0`` (or NaN), saturating."""
    value = offset / MASK_PITCH
    if value != value:
        return 0
    return int(value) if value < MASK_GRID else MASK_GRID


def in_mask(x: float, y: float) -> bool:
    """Whether the canvas point ``(x, y)`` lies inside the ``天`` mask."""
    if x < MASK_ORIGIN or y < MASK_ORIGIN:
        return False
    col = _lattice_index(x - MASK_ORIGIN)
    row = _lattice_index(y - MASK_ORIGIN)
    return col < MASK_GRID and row < MASK_GRID and MASK[row][col] == "#"


_CELL_LIGHT = tuple(in_mask(x, y) for x, y in zip(_CELL_X, _CELL_Y))


def cell_light(cell: int) -> bool:
    """Whether field cell ``cell`` lies inside the ``天`` (drawn in the light colour)."""
    return _CELL_LIGHT[_check_cell(cell)]


#: Number of concentric dot rings.
RING_COUNT = 3
#: Ring radii in design units.
RING_RADII = (405.0, 440.0, 475.0)
#: Dot slots on each ring (all multiples of four so the three cardinal gates
#: sit exactly on a slot).
RING_SLOTS = (88, 100, 108)
#: Radius of a drawn ring dot.
DOT_RADIUS = 9.0
#: Total dot slots over the three rings.
TOTAL_SLOTS = RING_SLOTS[0] + RING_SLOTS[1] + RING_SLOTS[2]

#: Dots in each cardinal gate, per ring, for the right, bottom and left gates.
#: There is deliberately no gate at the top.
GATE_DOTS = ((1, 1, 3), (2, 2, 2), (1, 2, 2))

#: Number of lane ``D`` bits carried by the rings.
D_BITS = 240


class SlotKind(enum.Enum):
    """What a ring slot is used for."""

    #: A gate dot, always lit.
    GATE = "gate"
    #: A slot next to a gate, always dark.
    GUARD = "guard"
    #: Carries one data bit of lane ``D`` (see :attr:`SlotRole.bit`).
    DATA = "data"
    #: Unused data slot, always dark.
    SPARE = "spare"


@dataclass(frozen=True)
class SlotRole:
    """The role of one ring slot; ``bit`` is set only for data slots."""

    kind: SlotKind
    bit: Optional[int] = None


_GATE = SlotRole(SlotKind.GATE)
_GUARD = SlotRole(SlotKind.GUARD)
_SPARE = SlotRole(SlotKind.SPARE)


def ring_offset(ring: int) -> int:
    """Offset of ring ``ring`` inside the flat slot index space."""
    if ring == 0:
        return 0
    if ring == 1:
        return RING_SLOTS[0]
    return RING_SLOTS[0] + RING_SLOTS[1]


def _gate_slots(ring: int) -> Tuple[List[int], List[int]]:
    n = RING_SLOTS[ring]
    bases = (0, n // 4, n // 2)  # right, bottom, left
    gates: List[int] = []
    guards: List[int] = []
    for gate, base in enumerate(bases):
        count = GATE_DOTS[gate][ring]
        if count == 1:
            first, last = base, base
        elif count == 2:
            first, last = base, base + 1
        else:
            first, last = base + n - 1, base + 1
        span = (last + n - first) % n + 1
        for step in range(span):
            gates.append((first + step) % n)
        guards.append((first + n - 1) % n)
        guards.append((last + 1) % n)
    return gates, guards


def _build_roles() -> Tuple[SlotRole, ...]:
    roles = [_SPARE] * TOTAL_SLOTS
    for ring in range(RING_COUNT):
        gates, guards = _gate_slots(ring)
        offset = ring_offset(ring)
        for slot in guards:
            roles[offset + slot] = _GUARD
        for slot in gates:
            roles[offset + slot] = _GATE
    following = 0
    for index, role in enumerate(roles):
        if role is _SPARE and following < D_BITS:
            roles[index] = SlotRole(SlotKind.DATA, following)
            following += 1
    return tuple(roles)


_ROLES = _build_roles()
_DATA_SLOTS = tuple(
    index
    for _, index in sorted(
        (role.bit, index) for index, role in enumerate(_ROLES) if role.kind is SlotKind.DATA
    )
)
_GATE_FLAT = tuple(i for i, role in enumerate(_ROLES) if role.kind is SlotKind.GATE)
_GUARD_FLAT = tuple(i for i, role in enumerate(_ROLES) if role.kind is SlotKind.GUARD)


def slot_roles() -> Tuple[SlotRole, ...]:
    """The role of every ring slot in flat index order."""
    return _ROLES


def data_slots() -> Tuple[int, ...]:
    """Flat slot index of every lane ``D`` bit, in bit order."""
    return _DATA_SLOTS


def gate_slots() -> Tuple[int, ...]:
    """Flat indices of every gate slot, ascending."""
    return _GATE_FLAT


def guard_slots() -> Tuple[int, ...]:
    """Flat indices of every guard slot, ascending."""
    return _GUARD_FLAT


def split_slot(flat: int) -> Tuple[int, int]:
    """Split a flat slot index into ``(ring, slot)``."""
    if flat < RING_SLOTS[0]:
        return 0, flat
    if flat < RING_SLOTS[0] + RING_SLOTS[1]:
        return 1, flat - RING_SLOTS[0]
    return 2, flat - RING_SLOTS[0] - RING_SLOTS[1]


#: ``core::f32::consts::TAU`` (2 pi rounded to single precision).
_TAU_F32 = _f32(math.tau)


def _slot_center_f32(ring: int, slot: int) -> Tuple[float, float]:
    # Rust evaluates this in f32. Every product and quotient of single values is
    # computed exactly-then-rounded by the double operation followed by
    # _f32() (53 >= 2 * 24 + 2 bits), and cos/sin of a single value rounded to
    # single precision is the correctly rounded cosf/sinf.
    theta = _f32(_f32(_TAU_F32 * slot) / RING_SLOTS[ring])
    radius = RING_RADII[ring]
    return (
        _f32(CENTER + _f32(radius * _f32(math.cos(theta)))),
        _f32(CENTER + _f32(radius * _f32(math.sin(theta)))),
    )


_SLOT_CENTERS = tuple(
    _slot_center_f32(ring, slot) for ring in range(RING_COUNT) for slot in range(RING_SLOTS[ring])
)


def slot_center(ring: int, slot: int) -> Tuple[float, float]:
    """Canvas coordinates of the centre of slot ``slot`` on ring ``ring``.

    Slot ``0`` is at 3 o'clock and slots advance clockwise on the screen. The
    values reproduce the reference's single-precision computation.
    """
    if not 0 <= ring < RING_COUNT or not 0 <= slot < RING_SLOTS[ring]:
        raise IndexError("ring slot out of range")
    return _SLOT_CENTERS[ring_offset(ring) + slot]


#: Canvas coordinates of the four corner finders, clockwise from top-left.
FINDER_CENTERS = ((72.0, 72.0), (952.0, 72.0), (952.0, 952.0), (72.0, 952.0))
#: Radius of the finder's solid centre disc.
FINDER_CORE = 12.0
#: Number of petals of a finder blossom.
FINDER_PETALS = 5
#: Distance from the finder centre to each petal centre.
FINDER_PETAL_DISTANCE = 34.0
#: Radius of each petal.
FINDER_PETAL_RADIUS = 26.0
#: Radius of the notch cut into each petal tip.
FINDER_NOTCH_RADIUS = 6.0
#: Outer radius of a finder blossom (tip of a petal).
FINDER_OUTER = 60.0


def _petal_angles() -> Tuple[float, ...]:
    return tuple(
        -(math.pi / 2.0) + math.tau * petal / FINDER_PETALS for petal in range(FINDER_PETALS)
    )


_PETAL_ANGLES = _petal_angles()
_PETALS = tuple(
    (FINDER_PETAL_DISTANCE * math.cos(angle), FINDER_PETAL_DISTANCE * math.sin(angle))
    for angle in _PETAL_ANGLES
)
_NOTCHES = tuple(
    (FINDER_OUTER * math.cos(angle), FINDER_OUTER * math.sin(angle)) for angle in _PETAL_ANGLES
)


def finder_petal_centers() -> Tuple[Tuple[float, float], ...]:
    """Petal centres relative to the finder centre; the first petal points up."""
    return _PETALS


def finder_notch_centers() -> Tuple[Tuple[float, float], ...]:
    """Notch centres (one at each petal tip) relative to the finder centre."""
    return _NOTCHES


def finder_lit(dx: float, dy: float) -> bool:
    """Return whether the point ``(dx, dy)``, relative to a finder centre, is lit.

    A finder is a solid five-petal sakura blossom whose first petal points
    straight up. The petal notches are cosmetic; decoders only rely on the
    blossom being one large, isolated, roughly round blob.
    """
    if math.sqrt(dx * dx + dy * dy) <= FINDER_CORE:
        return True
    for (cx, cy), (nx, ny) in zip(_PETALS, _NOTCHES):
        ex = dx - cx
        ey = dy - cy
        if math.sqrt(ex * ex + ey * ey) <= FINDER_PETAL_RADIUS:
            ex = dx - nx
            ey = dy - ny
            return math.sqrt(ex * ex + ey * ey) > FINDER_NOTCH_RADIUS
    return False


_NOTCH_RADIUS_SQUARED = PETAL_NOTCH_RADIUS * PETAL_NOTCH_RADIUS


def petal_lit(dx: float, dy: float, angle: float) -> bool:
    """Return whether the point ``(dx, dy)``, relative to the centre of a petal that
    points along ``angle``, is lit.

    A petal is an ellipse of half-length :data:`PETAL_HALF_LENGTH` along ``angle`` and
    half-width :data:`PETAL_HALF_WIDTH`, minus the notch at its tip.
    """
    sin = math.sin(angle)
    cos = math.cos(angle)
    along = dx * cos + dy * sin
    across = -dx * sin + dy * cos
    u = along / PETAL_HALF_LENGTH
    v = across / PETAL_HALF_WIDTH
    if u * u + v * v > 1.0:
        return False
    tip = along - PETAL_NOTCH_DISTANCE
    return tip * tip + across * across > _NOTCH_RADIUS_SQUARED


# ---- the card: the square and a band of katakana tiles above and below it ---------------------

#: Side of a katakana tile (:mod:`iroha_petal.kana`).
KANA_TILE = 25.0
#: Pitch of the katakana tile lattice.
KANA_PITCH = 29.0
#: Side of a tile's glyph box, centred in the tile.
KANA_GLYPH_BOX = 23.0
#: Corner radius of a tile.
KANA_CORNER = 3.0
#: Tile rows of each band.
KANA_ROWS = 10
#: Tile columns of each band.
KANA_COLUMNS = 35
#: Gap between a band's lattice and the square.
KANA_GAP = 24.0
#: Margin outside a band's lattice.
KANA_MARGIN = 8.0
#: Height of a band with its gap and margin (322 units).
KANA_BAND_HEIGHT = KANA_MARGIN + KANA_ROWS * KANA_PITCH + KANA_GAP
#: Height of the card: the square with a band above and below it (1668 units).
CARD_HEIGHT = CANVAS + 2.0 * KANA_BAND_HEIGHT
#: Card ``y`` of the square's top edge: card coordinates are square coordinates plus this in
#: ``y``.
SQUARE_TOP = KANA_BAND_HEIGHT
#: Tile centres keep at least this distance from every finder centre: the blossom's radius, a
#: blossom diameter of clearance, and a tile's half-diagonal, so no tile looks like part of a
#: finder.
KANA_FINDER_CLEARANCE = 198.0
#: Bands: above the square and below it.
KANA_BANDS = 2
#: Pilot tiles of each band.
KANA_PILOTS = 8

#: The pilot tiles of each band as ``(row, column)`` (band 1 mirrors band 0's rows, so the
#: pilots sit at the same distances from the square).
KANA_PILOT_CELLS = (
    ((0, 0), (0, 17), (0, 34), (4, 0), (4, 34), (9, 8), (9, 17), (9, 26)),
    ((9, 0), (9, 17), (9, 34), (5, 0), (5, 34), (0, 8), (0, 17), (0, 26)),
)
#: What every pilot shows: ``(glyph, light)`` in pilot order (ロ ケ ヒ ム ス ワ ア カ).
KANA_PILOT_SYMBOLS = (
    (1, True),
    (11, False),
    (13, True),
    (9, False),
    (14, False),
    (6, True),
    (12, False),
    (7, True),
)


@dataclass(frozen=True)
class KanaTile:
    """One katakana tile."""

    #: Band: 0 above the square, 1 below it.
    band: int
    #: Row within its band, top first.
    row: int
    #: Column, left first.
    column: int
    #: Left edge of the tile square in square coordinates.
    x: float
    #: Top edge of the tile square in square coordinates.
    y: float
    #: Index among the pilots of its band, when it is one.
    pilot: Optional[int] = None

    def centre(self) -> Tuple[float, float]:
        """Centre of the tile in square coordinates."""
        half = KANA_TILE / 2.0
        return self.x + half, self.y + half


#: Left edge of the lattice (the columns are centred on the card).
_KANA_LATTICE_LEFT = (CANVAS - KANA_COLUMNS * KANA_PITCH) / 2.0
#: Top of each band's lattice in square coordinates.
_KANA_LATTICE_TOP = (-KANA_BAND_HEIGHT + KANA_MARGIN, CANVAS + KANA_GAP)
#: The tile square sits this far inside its lattice cell.
_KANA_INSET = (KANA_PITCH - KANA_TILE) / 2.0


def _build_kana_tiles() -> Tuple[KanaTile, ...]:
    clearance = KANA_FINDER_CLEARANCE
    half = KANA_TILE / 2.0
    tiles = []
    for band in range(KANA_BANDS):
        for row in range(KANA_ROWS):
            for column in range(KANA_COLUMNS):
                x = _KANA_LATTICE_LEFT + KANA_PITCH * column + _KANA_INSET
                y = _KANA_LATTICE_TOP[band] + KANA_PITCH * row + _KANA_INSET
                cx = x + half
                cy = y + half
                if not all(
                    (cx - fx) * (cx - fx) + (cy - fy) * (cy - fy) >= clearance * clearance
                    for fx, fy in FINDER_CENTERS
                ):
                    continue
                cells = KANA_PILOT_CELLS[band]
                pilot = cells.index((row, column)) if (row, column) in cells else None
                tiles.append(KanaTile(band, row, column, x, y, pilot))
    return tuple(tiles)


_KANA_TILES = _build_kana_tiles()
_KANA_DATA_TILES = tuple(
    tuple(i for i, tile in enumerate(_KANA_TILES) if tile.band == band and tile.pilot is None)
    for band in range(KANA_BANDS)
)
_KANA_PILOT_TILES = tuple(
    tuple(
        next(i for i, tile in enumerate(_KANA_TILES) if tile.band == band and tile.pilot == pilot)
        for pilot in range(KANA_PILOTS)
    )
    for band in range(KANA_BANDS)
)


def _build_kana_index() -> Tuple[Optional[int], ...]:
    index: List[Optional[int]] = [None] * (KANA_BANDS * KANA_ROWS * KANA_COLUMNS)
    for i, tile in enumerate(_KANA_TILES):
        index[(tile.band * KANA_ROWS + tile.row) * KANA_COLUMNS + tile.column] = i
    return tuple(index)


_KANA_INDEX = _build_kana_index()


def _check_band(band: int) -> int:
    if not 0 <= band < KANA_BANDS:
        raise IndexError("band index out of range")
    return band


def kana_tiles() -> Tuple[KanaTile, ...]:
    """Every tile of both bands: band 0 first, each in reading order (rows top first, columns
    left first). A lattice cell whose tile centre lies within :data:`KANA_FINDER_CLEARANCE` of
    a finder centre has no tile (306 tiles per band remain)."""
    return _KANA_TILES


def kana_data_tiles(band: int) -> Tuple[int, ...]:
    """The data tiles of ``band`` (its tiles that are not pilots) in reading order, as
    indices into :func:`kana_tiles`."""
    return _KANA_DATA_TILES[_check_band(band)]


def kana_pilot_tiles(band: int) -> Tuple[int, ...]:
    """The pilot tiles of ``band`` in pilot order, as indices into :func:`kana_tiles`."""
    return _KANA_PILOT_TILES[_check_band(band)]


def kana_tile_at(x: float, y: float) -> Optional[Tuple[int, float, float]]:
    """The tile under the square-coordinate point ``(x, y)`` and the point's position inside
    the tile's square (``0 ... KANA_TILE`` on both axes), or ``None`` outside every tile
    square."""
    if y < 0.0:
        band = 0
    elif y >= CANVAS:
        band = 1
    else:
        return None
    gx = (x - _KANA_LATTICE_LEFT) / KANA_PITCH
    gy = (y - _KANA_LATTICE_TOP[band]) / KANA_PITCH
    if gx < 0.0 or gy < 0.0:
        return None
    # `floor() as usize`, then the lattice bounds (NaN and infinities fall outside)
    if not (gx < KANA_COLUMNS and gy < KANA_ROWS):
        return None
    column = int(gx)
    row = int(gy)
    tile = _KANA_INDEX[(band * KANA_ROWS + row) * KANA_COLUMNS + column]
    if tile is None:
        return None
    u = (gx - column) * KANA_PITCH - _KANA_INSET
    v = (gy - row) * KANA_PITCH - _KANA_INSET
    if 0.0 <= u <= KANA_TILE and 0.0 <= v <= KANA_TILE:
        return tile, u, v
    return None


def kana_inside(u: float, v: float) -> bool:
    """Whether the tile-local point ``(u, v)`` lies inside the rounded tile square."""
    side = KANA_TILE
    corner = KANA_CORNER
    if not (0.0 <= u <= side and 0.0 <= v <= side):
        return False
    cx = corner if u < corner else (side - corner if u > side - corner else u)
    cy = corner if v < corner else (side - corner if v > side - corner else v)
    dx = u - cx
    dy = v - cy
    return dx * dx + dy * dy <= corner * corner


#: The glyph box sits this far inside the tile square.
_KANA_GLYPH_INSET = (KANA_TILE - KANA_GLYPH_BOX) / 2.0
#: Design-grid units per tile unit.
_KANA_GLYPH_SCALE = GLYPH_GRID / KANA_GLYPH_BOX


def kana_inked(glyph: int, u: float, v: float) -> bool:
    """Whether the tile-local point ``(u, v)`` lies on ``glyph``'s strokes: inside the tile
    square and inked in the glyph box (inset by half the difference of tile and box)."""
    return kana_inside(u, v) and is_inked(
        glyph,
        (u - _KANA_GLYPH_INSET) * _KANA_GLYPH_SCALE,
        (v - _KANA_GLYPH_INSET) * _KANA_GLYPH_SCALE,
    )
