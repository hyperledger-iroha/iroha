# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Normative frame geometry.

All coordinates are *design units* on a square canvas of :data:`CANVAS` units
with the origin at the top-left and ``y`` growing downward. A renderer scales
the canvas to any pixel size; a decoder maps camera pixels back to design
units. Nothing here depends on floating-point rounding for bit placement: cell
*order* is defined by integer indices only.

The Rust reference stores the layout constants as ``f32``. Every constant is an
exact small integer or half-integer, so plain Python floats hold the same
values; only :func:`slot_center` evaluates trigonometry and therefore emulates
single precision explicitly.
"""

from __future__ import annotations

import enum
import math
import struct
from dataclasses import dataclass
from typing import List, Optional, Tuple

__all__ = [
    "CANVAS",
    "CENTER",
    "TILE_GRID",
    "TILE_ORIGIN",
    "TILE_PITCH",
    "TILE_SIZE",
    "GLYPH_BOX",
    "TILE_COUNT",
    "MASK",
    "TILES",
    "tile_center",
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
]

#: Canvas side length in design units.
CANVAS = 1024.0
#: Canvas centre coordinate on both axes.
CENTER = 512.0

#: Number of tile lattice columns and rows.
TILE_GRID = 20
#: Canvas coordinate of the lattice's left and top edge.
TILE_ORIGIN = 222.0
#: Lattice pitch in design units.
TILE_PITCH = 29.0
#: Side of a drawn tile in design units (pitch minus a 4-unit gutter).
TILE_SIZE = 25.0
#: Side of the square glyph box inside a tile.
GLYPH_BOX = 23.0
#: Number of data tiles in the ``天`` mask.
TILE_COUNT = 256

#: The ``天`` silhouette, one string per lattice row from top to bottom.
#: ``#`` marks a data tile. The mask is mirror-symmetric left to right and not
#: symmetric top to bottom, so it also tells a decoder which way is up.
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


def _build_tiles() -> Tuple[Tuple[int, int], ...]:
    tiles = [
        (col, row) for row, line in enumerate(MASK) for col, mark in enumerate(line) if mark == "#"
    ]
    if len(tiles) != TILE_COUNT:
        raise AssertionError("mask must contain exactly 256 tiles")
    return tuple(tiles)


#: Lattice ``(column, row)`` of every data tile in row-major order.
TILES = _build_tiles()


def tile_center(index: int) -> Tuple[float, float]:
    """Canvas coordinates of the centre of tile ``index``."""
    col, row = TILES[index]
    return (
        TILE_ORIGIN + TILE_PITCH * (col + 0.5),
        TILE_ORIGIN + TILE_PITCH * (row + 0.5),
    )


#: Number of concentric dot rings.
RING_COUNT = 3
#: Ring radii in design units.
RING_RADII = (360.0, 410.0, 460.0)
#: Dot slots on each ring (all multiples of four so the three cardinal gates
#: sit exactly on a slot).
RING_SLOTS = (80, 92, 104)
#: Radius of a drawn ring dot.
DOT_RADIUS = 11.0
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


_F32 = struct.Struct("<f")


def _f32(value: float) -> float:
    """Round a double to the nearest IEEE single-precision value."""
    return _F32.unpack(_F32.pack(value))[0]


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
