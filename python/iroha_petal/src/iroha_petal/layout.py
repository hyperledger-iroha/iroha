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
"""

from __future__ import annotations

import enum
import math
import struct
from bisect import bisect_right
from dataclasses import dataclass
from typing import List, Optional, Tuple

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
