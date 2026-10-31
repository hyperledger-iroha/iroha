# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The turn layer: data in the way the lit petals point.

A field lane tells the decoder exactly which petals of its sector are lit. The
first :data:`TURN_CARRIERS` lit petals of the sector, in the fixed
pseudo-random :func:`carrier_order`, carry two more bits each: the petal is
turned about its own centre by 0, 45, 90 or 135 degrees from its outward
direction (:func:`turn_angle`). A turned petal stays inside its own cell, so it
moves no light toward its neighbours and the field lanes read as before; a
camera that resolves the petals' shapes also reads the sector's turn lane.

The turn lane of a sector is one Reed-Solomon codeword, RS(90, 68),
XOR-whitened: four fountain atoms from the repair range (ids from
:data:`TURN_ID_BASE`, see :func:`turn_first_id`) and a CRC-32C check that binds
them to the stream tag, frame and sector of the field lane they ride on
(:func:`turn_check`). Carrier ``j`` shows bits ``2j`` and ``2j + 1`` of the
transmitted codeword (most significant bit first) as a Gray-coded position
(:func:`turn_position`). A sector with fewer than :data:`TURN_CARRIERS` lit
petals turns none (about one sector in 60,000).

:func:`read_turns` reads the turn lanes of a decoded capture: every carrier's
neighbourhood is sampled in the petal's own outward frame, following a smooth
correction of the pose fitted to the carriers' centroids; the blur is the one
under which the carriers and their lit neighbours explain the image best; and
each carrier takes the turn whose (slightly wider) template matches best once
its lit neighbours, at their own decided turns, are subtracted.

Every floating-point expression follows the reference operation by operation in
IEEE double precision, including the order of every sum (sums run through
:func:`functools.reduce`: Python's built-in ``sum`` of floats is compensated
from 3.12 on). The reader's tables are built once, on first use, and a
carrier's window is sampled only when the reader first needs it, so a capture
too blurred for turns pays for little more than the blur fit.
"""

from __future__ import annotations

import math
import threading
from dataclasses import dataclass
from functools import reduce
from itertools import islice
from operator import add, mul
from typing import Dict, List, Optional, Sequence, Tuple

from ._numeric import F64_MAX, NAN, f64_max, f64_min, total_key
from .crc import crc32c
from .decode import DecodedFrame, LaneResult
from .image import Luma, Pose
from .lanes import ATOM_LEN, FIELD_LANES, FIELD_WORD, encode_lane
from .layout import (
    BAND_COUNT,
    BAND_OFFSETS,
    FIELD_CELLS,
    SECTORS,
    cell_angle,
    cell_band_slot,
    cell_center,
    petal_lit,
    sector_cells,
)
from .prng import Xorshift32
from .rs import ReedSolomon, RsError, RsErrorKind
from .stream import AtomPacket, LaneHeader

__all__ = [
    "TURN_POSITIONS",
    "TURN_BITS",
    "TURN_CARRIERS",
    "TURN_WORD",
    "TURN_PARITY",
    "TURN_DATA",
    "TURN_ATOMS",
    "TURN_CHECK_LEN",
    "TURN_ID_BASE",
    "TURN_BLURS",
    "turn_angle",
    "turn_label",
    "turn_position",
    "carrier_order",
    "carriers",
    "turn_first_id",
    "turn_check",
    "turn_data",
    "turn_packet",
    "turn_whitening",
    "encode_turn_lane",
    "decode_turn_lane_counted",
    "carrier_position",
    "turn_template",
    "read_turns",
    "ObservedTurns",
    "observed_turns",
]

#: Turn positions a carrier petal can take.
TURN_POSITIONS = 4
#: Bits one carrier shows.
TURN_BITS = 2
#: Carrier petals of a sector.
TURN_CARRIERS = 360
#: Codeword length of a turn lane in bytes.
TURN_WORD = TURN_CARRIERS * TURN_BITS // 8
#: Parity bytes of a turn lane.
TURN_PARITY = 22
#: Data bytes of a turn lane: the atoms, then their check.
TURN_DATA = TURN_WORD - TURN_PARITY
#: Atoms carried by each turn lane.
TURN_ATOMS = 4
#: Bytes of the check that closes a turn lane's data.
TURN_CHECK_LEN = 4
#: Fountain id of the first turn atom of frame 0. The field lanes and lane ``D`` number
#: their atoms from 0 and never come near it.
TURN_ID_BASE = 1 << 31
#: Gaussian blurs (sigma in canvas units) the turn reader fits.
TURN_BLURS = (1.0, 1.4, 1.9, 2.5, 3.2, 4.0)

assert TURN_DATA == TURN_ATOMS * ATOM_LEN + TURN_CHECK_LEN
assert TURN_CARRIERS <= FIELD_WORD * 8
assert 1 << TURN_BITS == TURN_POSITIONS
assert (TURN_CARRIERS * TURN_BITS) % 8 == 0

_ATOM_BYTES = TURN_ATOMS * ATOM_LEN
_FIELD_BITS = FIELD_WORD * 8
#: ``turn_angle`` of every position: ``pi * position / 4``.
_TURN_ANGLES = tuple(math.pi * position / TURN_POSITIONS for position in range(TURN_POSITIONS))
#: ``turn_label`` of every position.
_TURN_LABELS = tuple(position ^ (position >> 1) for position in range(TURN_POSITIONS))


def _check_position(value: int, name: str) -> int:
    if not 0 <= value < TURN_POSITIONS:
        raise ValueError(f"{name} must be within 0..=3")
    return value


def _check_sector(sector: int) -> int:
    if not 0 <= sector < SECTORS:
        raise IndexError("sector out of range")
    return sector


def turn_angle(position: int) -> float:
    """The angle (radians, clockwise on screen) by which a petal at turn position
    ``position`` is turned from its outward direction: ``pi * position / 4``."""
    return _TURN_ANGLES[_check_position(position, "turn position")]


def turn_label(position: int) -> int:
    """The two bits a turn position shows: its Gray code ``position ^ (position >> 1)``.

    Neighbouring positions (135 and 0 degrees neighbour too, a petal turned by 180
    degrees differing only by its notch) differ in one bit.
    """
    return _TURN_LABELS[_check_position(position, "turn position")]


def turn_position(label: int) -> int:
    """The turn position that shows the two bits ``label`` (the inverse of
    :func:`turn_label`)."""
    return _TURN_LABELS[_check_position(label, "turn label")]


def _carrier_orders() -> Tuple[Tuple[int, ...], ...]:
    orders = []
    for sector in range(SECTORS):
        order = list(sector_cells(sector)[:_FIELD_BITS])
        rng = Xorshift32(0x4F524430 + sector)  # "ORD0" ... "ORD3"
        for i in range(len(order) - 1, 0, -1):
            j = rng.next_u32() % (i + 1)
            order[i], order[j] = order[j], order[i]
        orders.append(tuple(order))
    return tuple(orders)


_CARRIER_ORDERS = _carrier_orders()


def carrier_order(sector: int) -> Tuple[int, ...]:
    """The order in which the lit petals of sector ``sector`` become carriers.

    The sector's codeword cells (the first ``8 * FIELD_WORD`` of
    :func:`~iroha_petal.layout.sector_cells`) shuffled by Fisher-Yates with a
    :class:`~iroha_petal.prng.Xorshift32` seeded ``0x4F524430 + sector`` ("ORD0" ...
    "ORD3"): for ``i`` from 839 down to 1, position ``i`` swaps with position
    ``next_u32() mod (i + 1)``.
    """
    return _CARRIER_ORDERS[_check_sector(sector)]


def carriers(petals: Sequence[bool], sector: int) -> Optional[Tuple[int, ...]]:
    """The carriers of sector ``sector`` under the lit map ``petals`` (one flag per field
    cell): the first :data:`TURN_CARRIERS` lit cells in :func:`carrier_order`, or ``None``
    when fewer are lit."""
    order = _CARRIER_ORDERS[_check_sector(sector)]
    if len(petals) < FIELD_CELLS:
        raise ValueError("petals must hold one flag per field cell")
    chosen = tuple(islice((cell for cell in order if petals[cell]), TURN_CARRIERS))
    return chosen if len(chosen) == TURN_CARRIERS else None


def turn_first_id(frame: int, sector: int) -> int:
    """Fountain id of the first of the :data:`TURN_ATOMS` atoms the turn lane of
    ``sector`` carries in ``frame``: ``2^31 + 16 * frame + 4 * sector``."""
    if not 0 <= frame <= 0xFFFF:
        raise ValueError("frame counters are 16-bit unsigned integers")
    return TURN_ID_BASE + frame * (SECTORS * TURN_ATOMS) + _check_sector(sector) * TURN_ATOMS


def turn_check(header: LaneHeader, sector: int, atoms: bytes) -> int:
    """The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the
    sector and the atoms, so a turn lane read under the wrong field lane, frame or stream
    is refused."""
    frame = header.frame
    return crc32c(bytes((header.tag, frame >> 8, frame & 0xFF, sector)) + bytes(atoms))


def turn_data(header: LaneHeader, sector: int, atoms: bytes) -> bytes:
    """The data of a turn lane: the atoms followed by their :func:`turn_check`
    (big-endian).

    Raises :class:`ValueError` when ``atoms`` is not :data:`TURN_ATOMS` atoms long.
    """
    atoms = bytes(atoms)
    if len(atoms) != _ATOM_BYTES:
        raise ValueError("turn atoms length mismatch")
    _check_sector(sector)
    return atoms + turn_check(header, sector, atoms).to_bytes(TURN_CHECK_LEN, "big")


def turn_packet(header: LaneHeader, sector: int, data: bytes) -> Optional[AtomPacket]:
    """The atoms of turn lane data whose check matches ``header`` and ``sector``;
    ``None`` for data of the wrong length, a sector out of range or a check that does not
    match."""
    if len(data) != TURN_DATA or not 0 <= sector < SECTORS:
        return None
    atoms = bytes(data[:_ATOM_BYTES])
    if turn_check(header, sector, atoms).to_bytes(TURN_CHECK_LEN, "big") != bytes(
        data[_ATOM_BYTES:]
    ):
        return None
    return AtomPacket(
        header=header,
        first_id=turn_first_id(header.frame, sector),
        atoms=tuple(atoms[i * ATOM_LEN : (i + 1) * ATOM_LEN] for i in range(TURN_ATOMS)),
    )


def _whitening(sector: int) -> bytes:
    rng = Xorshift32(0x54524E30 + sector)  # "TRN0" ... "TRN3"
    return bytes(rng.next_byte() for _ in range(TURN_WORD))


_WHITENING = tuple(_whitening(sector) for sector in range(SECTORS))
_CODE = ReedSolomon(TURN_PARITY)


def turn_whitening(sector: int) -> bytes:
    """The whitening sequence of the turn lane of ``sector``: one byte per codeword byte
    from a :class:`~iroha_petal.prng.Xorshift32` seeded ``0x54524E30 + sector`` ("TRN0"
    ... "TRN3")."""
    return _WHITENING[_check_sector(sector)]


def encode_turn_lane(sector: int, data: bytes) -> bytes:
    """Encode turn lane data into the transmitted (whitened) codeword.

    Raises :class:`ValueError` when ``data`` is not exactly :data:`TURN_DATA` bytes.
    """
    whitening = _WHITENING[_check_sector(sector)]
    if len(data) != TURN_DATA:
        raise ValueError("turn data length mismatch")
    word = _CODE.encode(bytes(data))
    return bytes(a ^ b for a, b in zip(word, whitening))


def decode_turn_lane_counted(
    sector: int, transmitted: bytes, erasures: Sequence[int] = ()
) -> Tuple[bytes, int]:
    """Decode a transmitted turn codeword, returning its data and how many byte positions
    the Reed-Solomon decoder rewrote. The check is not verified here (see
    :func:`turn_packet`).

    Raises :class:`~iroha_petal.rs.RsError` when the word is uncorrectable.
    """
    whitening = _WHITENING[_check_sector(sector)]
    if len(transmitted) != TURN_WORD:
        raise RsError(RsErrorKind.INVALID_SHAPE)
    word = bytearray(a ^ b for a, b in zip(transmitted, whitening))
    corrected = _CODE.decode(word, erasures)
    return bytes(word[:TURN_DATA]), corrected


def carrier_position(word: bytes, index: int) -> int:
    """The turn position carrier ``index`` of a transmitted turn codeword shows."""
    if not 0 <= index < len(word) * 4:
        raise IndexError("carrier index out of range")
    return _TURN_LABELS[(word[index >> 2] >> (6 - 2 * (index & 3))) & 3]


# ---- reading ------------------------------------------------------------------------------

#: Half the side, in canvas units, of the square a petal template is tabulated over.
_TEMPLATE_HALF = 9.0
#: Template table step in canvas units.
_TEMPLATE_STEP = 0.25
#: Template table side in steps.
_TEMPLATE_SIDE = 73
#: The last table index, as the bound of the bilinear interpolation.
_TEMPLATE_LAST = float(_TEMPLATE_SIDE - 1)
#: From this fitted blur on (3.2 units), the camera cannot resolve the turns and the reader
#: stops.
_BLUR_LIMIT = 4
#: The window of a carrier is sampled every canvas unit over +-``_GRID_HALF`` units in its
#: own frame.
_GRID_HALF = 6
#: Window side in samples.
_GRID_SIDE = 13
#: Samples of a window.
_GRID_POINTS = _GRID_SIDE * _GRID_SIDE
#: Sigma in canvas units of the Gaussian that weights the window.
_WINDOW_SIGMA = 3.6
#: Sigma in canvas units of the Gaussian that follows a carrier's centroid.
_CENTROID_SIGMA = 2.6
#: Lit petals within this distance of a carrier are modelled as its neighbours.
_NEIGHBOUR_DISTANCE = 19.0
#: A neighbour's template is subtracted within this distance of its centre. Radially
#: adjacent cell centres are 11 units apart and the window grid is integral, so a whole
#: number here would put grid points exactly on the boundary, where the last bit of a
#: platform's ``sin`` and ``cos`` decides.
_NEIGHBOUR_REACH = 9.5
#: The warp and the blur are fitted on every this-many-th carrier.
_SAMPLE_STEP = 17
#: A window's 10th percentile (the 17th smallest of 169 samples) becomes 0 ...
_WINDOW_LOW = _GRID_POINTS // 10
#: ... and its 98th (the 166th smallest) becomes 1.
_WINDOW_HIGH = _GRID_POINTS * 98 // 100
#: The window offsets ``-6 ... 6`` (canvas units) along and across, as floats.
_STEPS = tuple(float(t) for t in range(-_GRID_HALF, _GRID_HALF + 1))

_CELL_X = tuple(cell_center(cell)[0] for cell in range(FIELD_CELLS))
_CELL_Y = tuple(cell_center(cell)[1] for cell in range(FIELD_CELLS))
_CELL_ANGLE = tuple(cell_angle(cell) for cell in range(FIELD_CELLS))
_CELL_SIN = tuple(math.sin(angle) for angle in _CELL_ANGLE)
_CELL_COS = tuple(math.cos(angle) for angle in _CELL_ANGLE)
#: The codeword cells of every sector, in bit order.
_SECTOR_BITS = tuple(sector_cells(sector)[:_FIELD_BITS] for sector in range(SECTORS))
_NO_SKIP = (False,) * SECTORS
_EVERY_SECTOR = (True,) * SECTORS


def _template_at(table: Sequence[float], along: float, across: float) -> float:
    """A petal template at a point of its own frame (``along`` its axis, ``across`` it):
    the bilinear interpolation of ``table``, zero outside it."""
    x = (along + _TEMPLATE_HALF) / _TEMPLATE_STEP
    y = (across + _TEMPLATE_HALF) / _TEMPLATE_STEP
    if not (x >= 0.0 and y >= 0.0 and x < _TEMPLATE_LAST and y < _TEMPLATE_LAST):
        return 0.0
    i = int(x)
    j = int(y)
    fx = x - i
    fy = y - j
    row = j * _TEMPLATE_SIDE + i
    top = table[row] * (1.0 - fx) + table[row + 1] * fx
    bottom = table[row + _TEMPLATE_SIDE] * (1.0 - fx) + table[row + _TEMPLATE_SIDE + 1] * fx
    return top * (1.0 - fy) + bottom * fy


def _coverage() -> List[float]:
    """The share of every table cell the unturned petal covers (4 x 4 samples per
    cell)."""
    side = _TEMPLATE_SIDE
    offsets = [(s - 1.5) * _TEMPLATE_STEP / 4.0 for s in range(4)]
    cover = [0.0] * (side * side)
    for j in range(side):
        across = -_TEMPLATE_HALF + j * _TEMPLATE_STEP
        for i in range(side):
            along = -_TEMPLATE_HALF + i * _TEMPLATE_STEP
            hits = 0
            for dc in offsets:
                for da in offsets:
                    if petal_lit(along + da, across + dc, 0.0):
                        hits += 1
            cover[j * side + i] = hits / 16.0
    return cover


def _blurred(cover: Sequence[float], sigma: float) -> List[float]:
    """The coverage blurred by a Gaussian of ``sigma`` canvas units truncated at
    ``3 sigma``: rows, then columns; taps outside the table count as zero, and the kernel
    is normalised by its full sum."""
    side = _TEMPLATE_SIDE
    radius = math.ceil(3.0 * sigma / _TEMPLATE_STEP)
    taps = []
    for k in range(-radius, radius + 1):
        t = k * _TEMPLATE_STEP
        taps.append(math.exp(-(t * t) / (2.0 * sigma * sigma)))
    norm = reduce(add, taps)
    last_tap = 2 * radius

    def blur(source: Sequence[float], rows: bool) -> List[float]:
        out = [0.0] * (side * side)
        for j in range(side):
            for i in range(side):
                # the taps that land inside the table, in tap order
                at = i if rows else j
                first = max(radius - at, 0)
                last = min(radius + side - 1 - at, last_tap)
                if rows:
                    start = j * side + i - radius + first
                    values = source[start : start + last - first + 1]
                else:
                    start = (j - radius + first) * side + i
                    values = source[start : start + (last - first) * side + 1 : side]
                total = reduce(add, map(mul, taps[first : last + 1], values), 0.0)
                out[j * side + i] = total / norm
        return out

    return blur(blur(cover, True), False)


#: The window grid, ``(along, across)`` by rows across and columns along.
_GRID = tuple((a, b) for b in _STEPS for a in _STEPS)
#: The window grid's Gaussian weights, in grid order.
_WEIGHTS = tuple(
    math.exp(-(a * a + b * b) / (2.0 * _WINDOW_SIGMA * _WINDOW_SIGMA)) for a, b in _GRID
)


class _Tables:
    """Per blur: the template table, the template of every turn position on the window
    grid, and the weighted kernel of every turn position (the grid weight times that
    template)."""

    __slots__ = ("templates", "models", "kernels")

    def __init__(self) -> None:
        cover = _coverage()
        self.templates = tuple(_blurred(cover, sigma) for sigma in TURN_BLURS)
        models = []
        kernels = []
        for table in self.templates:
            turned = []
            for angle in _TURN_ANGLES:
                sin = math.sin(angle)
                cos = math.cos(angle)
                turned.append(
                    tuple(
                        _template_at(table, a * cos + b * sin, -a * sin + b * cos) for a, b in _GRID
                    )
                )
            models.append(tuple(turned))
            kernels.append(tuple(tuple(map(mul, _WEIGHTS, model)) for model in turned))
        self.models = tuple(models)
        self.kernels = tuple(kernels)


_lock = threading.Lock()
_tables_built: Optional[_Tables] = None
_neighbours_built: Optional[Tuple[Tuple[Tuple[int, float, float, float], ...], ...]] = None


def _tables() -> _Tables:
    """The reader's tables, built on first use."""
    global _tables_built
    tables = _tables_built
    if tables is None:
        with _lock:
            if _tables_built is None:
                _tables_built = _Tables()
            tables = _tables_built
    return tables


def _build_neighbours() -> Tuple[Tuple[Tuple[int, float, float, float], ...], ...]:
    # Cells two bands apart are 22 units apart or more, so only the band itself and the two
    # beside it can hold a cell within the 19 units; the lists are the reference's (which
    # scans two bands either side) entry for entry.
    limit = _NEIGHBOUR_DISTANCE * _NEIGHBOUR_DISTANCE
    xs, ys, angles = _CELL_X, _CELL_Y, _CELL_ANGLE
    table = []
    for cell in range(FIELD_CELLS):
        x = xs[cell]
        y = ys[cell]
        angle = angles[cell]
        sin = _CELL_SIN[cell]
        cos = _CELL_COS[cell]
        band, _ = cell_band_slot(cell)
        first = BAND_OFFSETS[max(band - 1, 0)]
        last = BAND_OFFSETS[min(band + 2, BAND_COUNT)]
        near = []
        for other in range(first, last):
            dx = xs[other] - x
            dy = ys[other] - y
            if dx * dx + dy * dy <= limit and other != cell:
                near.append(
                    (other, dx * cos + dy * sin, -dx * sin + dy * cos, angles[other] - angle)
                )
        table.append(tuple(near))
    return tuple(table)


def _neighbours() -> Tuple[Tuple[Tuple[int, float, float, float], ...], ...]:
    """The cells within 19 units of every field cell (itself excluded), in index order:
    ``(cell, along, across, angle)`` with the other centre and outward direction in the
    cell's own outward frame. Built on first use."""
    global _neighbours_built
    table = _neighbours_built
    if table is None:
        with _lock:
            if _neighbours_built is None:
                _neighbours_built = _build_neighbours()
            table = _neighbours_built
    return table


def turn_template(blur: int, along: float, across: float) -> float:
    """The turn reader's blurred petal template for blur ``blur`` (an index into
    :data:`TURN_BLURS`) at a point of the unturned petal's own frame (``along`` its axis,
    ``across`` it): what conformance tests compare an SDK's tables with."""
    if not 0 <= blur < len(TURN_BLURS):
        raise IndexError("blur index out of range")
    return _template_at(_tables().templates[blur], float(along), float(across))


def _sample_window(
    image: Luma, pose: Pose, cell: int, shift_x: float, shift_y: float
) -> Tuple[List[float], bool]:
    """Samples the image on the window grid of the petal at ``cell``, centred
    ``(shift_x, shift_y)`` canvas units off the cell centre, in the petal's outward frame
    (grid rows across, columns along); also says whether a sample is NaN (only a broken
    pose samples one).

    Within a window the pose is taken as affine: with ``o`` the image point of the centre
    and ``u``, ``v`` the image steps of one unit along and across, grid point ``(a, b)``
    is sampled at ``(o + b v) + a u`` (bilinearly, clamped to the border, as
    :meth:`~iroha_petal.image.Luma.sample`).
    """
    x = _CELL_X[cell] + shift_x
    y = _CELL_Y[cell] + shift_y
    sin = _CELL_SIN[cell]
    cos = _CELL_COS[cell]
    ox, oy = pose.apply(x, y)
    ax, ay = pose.apply(x + cos, y + sin)
    cx, cy = pose.apply(x - sin, y + cos)
    ux = ax - ox
    uy = ay - oy
    vx = cx - ox
    vy = cy - oy
    data = image.data
    w = image.width
    wm1 = w - 1
    hm1 = image.height - 1
    fwm1 = float(wm1)
    fhm1 = float(hm1)
    out: List[float] = []
    append = out.append
    # The samples are an affine image of the grid, so when the four corner samples lie a
    # pixel or more inside the image, every sample does: no clamp acts, no sample is NaN
    # and the right and lower neighbours always exist. The same arithmetic then runs
    # without the border cases.
    inside = True
    for b in (_STEPS[0], _STEPS[-1]):
        for a in (_STEPS[0], _STEPS[-1]):
            fx = ox + b * vx + a * ux - 0.5
            fy = oy + b * vy + a * uy - 0.5
            if not (1.0 <= fx <= fwm1 - 1.0 and 1.0 <= fy <= fhm1 - 1.0):
                inside = False
    if inside:
        for b in _STEPS:
            row_x = ox + b * vx
            row_y = oy + b * vy
            for a in _STEPS:
                fx = row_x + a * ux - 0.5
                fy = row_y + a * uy - 0.5
                x0 = int(fx)
                y0 = int(fy)
                tx = fx - x0
                ty = fy - y0
                r0 = y0 * w + x0
                r1 = r0 + w
                left = 1.0 - tx
                top = data[r0] * left + data[r0 + 1] * tx
                bottom = data[r1] * left + data[r1 + 1] * tx
                append(top * (1.0 - ty) + bottom * ty)
        return out, False
    nan = False
    for b in _STEPS:
        row_x = ox + b * vx
        row_y = oy + b * vy
        for a in _STEPS:
            fx = row_x + a * ux - 0.5
            fy = row_y + a * uy - 0.5
            if fx != fx or fy != fy:
                append(NAN)
                nan = True
                continue
            if fx < 0.0:
                fx = 0.0
            elif fx > fwm1:
                fx = fwm1
            if fy < 0.0:
                fy = 0.0
            elif fy > fhm1:
                fy = fhm1
            x0 = int(fx)
            y0 = int(fy)
            x1 = x0 + 1 if x0 < wm1 else wm1
            y1 = y0 + 1 if y0 < hm1 else hm1
            tx = fx - x0
            ty = fy - y0
            r0 = y0 * w
            r1 = y1 * w
            top = data[r0 + x0] * (1.0 - tx) + data[r0 + x1] * tx
            bottom = data[r1 + x0] * (1.0 - tx) + data[r1 + x1] * tx
            append(top * (1.0 - ty) + bottom * ty)
    return out, nan


def _normalise(window: Sequence[float], nan: bool) -> List[float]:
    """Rescales a window so that its 10th percentile (of 169 samples, the 17th smallest)
    becomes 0 and its 98th (the 166th smallest) 1 (a span below one grey level counts as
    one), clamped to -0.2 ... 1.2. A window holding NaN is ordered as Rust's
    ``f64::total_cmp`` orders it."""
    ordered = sorted(window, key=total_key) if nan else sorted(window)
    bottom = ordered[_WINDOW_LOW]
    span = ordered[_WINDOW_HIGH] - bottom
    if not span > 1.0:  # `f64::max(1.0)`: a NaN span yields 1.0
        span = 1.0
    return [
        -0.2 if (v := (value - bottom) / span) < -0.2 else (1.2 if v > 1.2 else v)
        for value in window
    ]


def _centroid(window: Sequence[float]) -> Tuple[float, float]:
    """The centroid (along, across) of the light of a normalised window: every sample
    weighs its level (clamped to 0 ... 1) times a Gaussian of sigma 2.6 that starts at the
    window centre and follows the centroid three times; the Gaussian is its along factor
    times its across factor."""
    ea = 0.0
    eb = 0.0
    k = 2.0 * _CENTROID_SIGMA * _CENTROID_SIGMA
    exp = math.exp
    for _ in range(3):
        along = [exp(-(t - ea) * (t - ea) / k) for t in _STEPS]
        across = [exp(-(t - eb) * (t - eb) / k) for t in _STEPS]
        total = 0.0
        sa = 0.0
        sb = 0.0
        index = 0
        for b, gb in zip(_STEPS, across):
            for a, ga in zip(_STEPS, along):
                level = window[index]
                index += 1
                weight = (0.0 if level < 0.0 else (1.0 if level > 1.0 else level)) * ga * gb
                total += weight
                sa += weight * a
                sb += weight * b
        if total <= 0.0:
            break
        ea = sa / total
        eb = sb / total
    return ea, eb


def _monomials(x: float, y: float) -> Tuple[float, ...]:
    """The ten cubic monomials of a canvas point, centred on the canvas and scaled by
    384."""
    x = (x - 512.0) / 384.0
    y = (y - 512.0) / 384.0
    return (1.0, x, y, x * x, x * y, y * y, x * x * x, x * x * y, x * y * y, y * y * y)


def _fit_cubic(points: Sequence[Tuple[float, float]], values: Sequence[float]) -> List[float]:
    """The least-squares cubic through ``values`` at ``points``: normal equations (summed
    point by point) with 1e-6 added to the diagonal, solved by Gauss-Jordan elimination
    with partial pivoting (a pivot below 1e-12 skips its column)."""
    a = [[0.0] * 11 for _ in range(10)]
    for (x, y), value in zip(points, values):
        f = _monomials(x, y)
        for r in range(10):
            row = a[r]
            fr = f[r]
            for c in range(10):
                row[c] += fr * f[c]
            row[10] += fr * value
    for r in range(10):
        a[r][r] += 1e-6
    for col in range(10):
        pivot = col
        for r in range(col + 1, 10):
            if abs(a[r][col]) > abs(a[pivot][col]):
                pivot = r
        a[col], a[pivot] = a[pivot], a[col]
        p = a[col][col]
        if abs(p) < 1e-12:
            continue
        pivot_row = a[col]
        for c in range(col, 11):
            pivot_row[c] /= p
        for r in range(10):
            if r != col:
                row = a[r]
                f = row[col]
                for c in range(col, 11):
                    row[c] -= f * pivot_row[c]
    return [a[r][10] for r in range(10)]


def _eval_cubic(coefficients: Sequence[float], x: float, y: float) -> float:
    return reduce(add, map(mul, _monomials(x, y), coefficients), -0.0)


def _add_neighbour(
    window: List[float],
    table: Sequence[float],
    along: float,
    across: float,
    angle: float,
    turn: float,
    sign: float,
) -> None:
    """Adds ``sign x`` the template of the neighbour centred at ``(along, across)`` with
    outward direction ``angle`` (in the window's frame), turned by ``turn``, to a window, at
    the grid points within 9.5 units of the neighbour's centre."""
    phi = angle + turn
    sin = math.sin(phi)
    cos = math.cos(phi)
    reach = _NEIGHBOUR_REACH
    reach2 = reach * reach
    # only the grid rows and columns within reach of the neighbour's centre
    a0 = max(math.ceil(along - reach), -_GRID_HALF)
    a1 = min(math.floor(along + reach), _GRID_HALF)
    b0 = max(math.ceil(across - reach), -_GRID_HALF)
    b1 = min(math.floor(across + reach), _GRID_HALF)
    side = _TEMPLATE_SIDE
    last = _TEMPLATE_LAST
    for b in range(b0, b1 + 1):
        ub = b - across
        ub2 = ub * ub
        ub_sin = ub * sin
        ub_cos = ub * cos
        row = (b + _GRID_HALF) * _GRID_SIDE + _GRID_HALF
        for a in range(a0, a1 + 1):
            ua = a - along
            if ua * ua + ub2 > reach2:
                continue
            # the template at (ua cos + ub sin, -ua sin + ub cos), inlined
            x = (ua * cos + ub_sin + _TEMPLATE_HALF) / _TEMPLATE_STEP
            y = (-ua * sin + ub_cos + _TEMPLATE_HALF) / _TEMPLATE_STEP
            if x >= 0.0 and y >= 0.0 and x < last and y < last:
                i = int(x)
                j = int(y)
                fx = x - i
                fy = y - j
                at = j * side + i
                value = (table[at] * (1.0 - fx) + table[at + 1] * fx) * (1.0 - fy) + (
                    table[at + side] * (1.0 - fx) + table[at + side + 1] * fx
                ) * fy
            else:
                value = 0.0
            window[row + a] += sign * value


def _decide(window: Sequence[float], kernels: Sequence[Sequence[float]]) -> Tuple[int, float]:
    """The best turn position for a window and how far its score leads the runner-up's
    (ties: the lower position)."""
    k0, k1, k2, k3 = kernels
    scores = (
        reduce(add, map(mul, window, k0), -0.0),
        reduce(add, map(mul, window, k1), -0.0),
        reduce(add, map(mul, window, k2), -0.0),
        reduce(add, map(mul, window, k3), -0.0),
    )
    best = 0
    for k in range(1, TURN_POSITIONS):
        if scores[k] > scores[best]:
            best = k
    runner_up = -math.inf
    for k in range(TURN_POSITIONS):
        if k != best:
            runner_up = f64_max(runner_up, scores[k])
    return best, scores[best] - runner_up


@dataclass(frozen=True)
class _Reading:
    """What the turn reader decided for one capture."""

    headers: Tuple[Optional[LaneHeader], ...]
    #: ``(start, stop)`` of every sector's carriers in ``carriers``.
    ranges: Tuple[Optional[Tuple[int, int]], ...]
    carriers: Tuple[int, ...]
    decided: Tuple[Tuple[int, float], ...]
    fitted: int


def _readable(image: Luma) -> bool:
    """Whether the plane can be sampled (a plane whose buffer does not match its size is
    read as nothing)."""
    return image.width > 0 and image.height > 0 and len(image.data) == image.width * image.height


def _decide_turns(
    image: Luma, frame: DecodedFrame, need: Sequence[bool], limit: bool
) -> Optional[_Reading]:
    """Decides the turn of every carrier of the sectors ``frame`` decoded, when a sector
    that ``need`` names has carriers; with ``limit``, not when the fitted blur reaches
    3.2 units.

    1. The lit petals of the decoded sectors come from their field lanes, and the carriers
       of each from :func:`carriers`.
    2. The pose's smooth error: for every 17th carrier, the window is sampled at its cell
       centre and normalised, and its centroid turned into a canvas offset; a cubic fitted
       to the offsets over the canvas moves every carrier's window, which is then sampled
       and normalised.
    3. The blur: for every 17th carrier, the model of the petal at its best turn plus its
       lit neighbours (within 19 units) at theirs (each decided on its own window,
       unturned when it carries nothing) is correlated with the window, weighted by the
       window Gaussian; the blur with the largest sum fits best.
    4. The turns: with the next wider blur's kernels, every carrier takes the best turn of
       its own window, then, carrier by carrier in order, the best turn of its window with
       its lit neighbours' templates at their latest turns subtracted.
    """
    # the lit petals of every sector that decoded
    lit = bytearray(FIELD_CELLS)
    headers: List[Optional[LaneHeader]] = [None] * SECTORS
    for sector, field in enumerate(frame.fields):
        if field is None or len(field.data) < 3:
            continue
        data = field.data
        headers[sector] = LaneHeader(data[0], (data[1] << 8) | data[2])
        word = encode_lane(FIELD_LANES[sector], data)
        for bit, cell in enumerate(_SECTOR_BITS[sector]):
            lit[cell] = (word[bit >> 3] >> (7 - (bit & 7))) & 1
    # the carriers of every decoded sector, sector by sector in carrier order
    carrier_cells: List[int] = []
    ranges: List[Optional[Tuple[int, int]]] = [None] * SECTORS
    for sector in range(SECTORS):
        if headers[sector] is None:
            continue
        chosen = carriers(lit, sector)
        if chosen is not None:
            start = len(carrier_cells)
            carrier_cells.extend(chosen)
            ranges[sector] = (start, len(carrier_cells))
    if all(not need[s] or ranges[s] is None for s in range(SECTORS)):
        return None
    tables = _tables()
    neighbours = _neighbours()
    pose = frame.pose
    count = len(carrier_cells)
    sampled = range(0, count, _SAMPLE_STEP)
    # the pose's smooth error, from the centroids of a sample of the carriers
    points = []
    offsets_x = []
    offsets_y = []
    for index in sampled:
        cell = carrier_cells[index]
        a, b = _centroid(_normalise(*_sample_window(image, pose, cell, 0.0, 0.0)))
        sin = _CELL_SIN[cell]
        cos = _CELL_COS[cell]
        points.append((_CELL_X[cell], _CELL_Y[cell]))
        offsets_x.append(a * cos - b * sin)
        offsets_y.append(a * sin + b * cos)
    fit_x = _fit_cubic(points, offsets_x)
    fit_y = _fit_cubic(points, offsets_y)
    # every carrier's window, centred on its cell centre plus the fitted offset; sampled
    # when first needed (the result does not depend on when)
    windows: List[Optional[List[float]]] = [None] * count

    def window_of(index: int) -> List[float]:
        window = windows[index]
        if window is None:
            cell = carrier_cells[index]
            x = _CELL_X[cell]
            y = _CELL_Y[cell]
            shift_x = _eval_cubic(fit_x, x, y)
            shift_y = _eval_cubic(fit_y, x, y)
            window = _normalise(*_sample_window(image, pose, cell, shift_x, shift_y))
            windows[index] = window
        return window

    carrier_of = [-1] * FIELD_CELLS
    for index, cell in enumerate(carrier_cells):
        carrier_of[cell] = index

    def turn_of(carrier: int, kernels: Sequence[Sequence[float]], known: Dict[int, int]) -> int:
        """The best turn of a carrier's own window under ``kernels`` (remembered in
        ``known``: it is the same every time)."""
        position = known.get(carrier)
        if position is None:
            position = _decide(window_of(carrier), kernels)[0]
            known[carrier] = position
        return position

    # the blur under which a sample of the carriers, each at its best turn with its lit
    # neighbours at theirs, explains the windows best
    fitted = 0
    best_score = -math.inf
    spread: Dict[int, float] = {}
    for blur in range(len(TURN_BLURS)):
        table = tables.templates[blur]
        kernels = tables.kernels[blur]
        models = tables.models[blur]
        known: Dict[int, int] = {}
        score = 0.0
        for i in sampled:
            model = list(models[turn_of(i, kernels, known)])
            for other, along, across, angle in neighbours[carrier_cells[i]]:
                if not lit[other]:
                    continue
                j = carrier_of[other]
                turn = 0.0 if j < 0 else _TURN_ANGLES[turn_of(j, kernels, known)]
                _add_neighbour(model, table, along, across, angle, turn, 1.0)
            window = window_of(i)
            cross = reduce(add, map(mul, map(mul, _WEIGHTS, window), model), 0.0)
            mm = reduce(add, map(mul, map(mul, _WEIGHTS, model), model), 0.0)
            xx = spread.get(i)
            if xx is None:
                xx = reduce(add, map(mul, map(mul, _WEIGHTS, window), window), 0.0)
                spread[i] = xx
            root = math.sqrt(mm * xx)
            score += cross / (root if root > 1e-9 else 1e-9)  # `f64::max(1e-9)`
        if score > best_score:
            best_score = score
            fitted = blur
    if limit and fitted >= _BLUR_LIMIT:
        return None
    # the turns, decided with the next wider blur
    blur = min(fitted + 1, len(TURN_BLURS) - 1)
    table = tables.templates[blur]
    kernels = tables.kernels[blur]
    decided = [_decide(window_of(index), kernels) for index in range(count)]
    for index, cell in enumerate(carrier_cells):
        scratch = list(window_of(index))
        for other, along, across, angle in neighbours[cell]:
            if not lit[other]:
                continue
            j = carrier_of[other]
            turn = 0.0 if j < 0 else _TURN_ANGLES[decided[j][0]]
            _add_neighbour(scratch, table, along, across, angle, turn, -1.0)
        decided[index] = _decide(scratch, kernels)
    return _Reading(
        headers=tuple(headers),
        ranges=tuple(ranges),
        carriers=tuple(carrier_cells),
        decided=tuple(decided),
        fitted=fitted,
    )


def _decode_turn_word(
    sector: int, header: LaneHeader, word: bytes, confidence: Sequence[float]
) -> Optional[LaneResult]:
    """Tries Reed-Solomon with 0, 2, 5, 7 and 11 of the least confident bytes erased (ties
    keep byte order) and accepts the first result whose check matches."""
    order = sorted(range(len(word)), key=lambda i: total_key(confidence[i]))
    nsym = TURN_PARITY
    schedule: List[int] = []
    for count in (0, nsym // 8, nsym // 4, nsym // 3, nsym // 2):
        if not schedule or schedule[-1] != count:
            schedule.append(count)
    for erasures in schedule:
        positions = order[:erasures]
        trial = bytearray(word)
        for p in positions:
            trial[p] = 0
        try:
            data, corrected = decode_turn_lane_counted(sector, trial, positions)
        except RsError:
            continue
        if turn_packet(header, sector, data) is not None:
            return LaneResult(data=data, corrected=corrected, erasures=len(positions))
    return None


def read_turns(
    image: Luma, frame: DecodedFrame, skip: Sequence[bool] = _NO_SKIP
) -> Tuple[Optional[LaneResult], ...]:
    """Read the turn lanes of the sectors ``frame`` decoded from ``image``, except those in
    ``skip`` (one flag per sector).

    The carriers of every decoded sector are decided (so that each carrier's neighbours
    are modelled at their own turns), but only the lanes of the sectors not in ``skip``
    are decoded, and none when the image is too blurred to show turns. A lane passes when
    its Reed-Solomon codeword decodes, with 0, 2, 5, 7 or 11 of its least confident bytes
    erased (a byte's confidence is the smallest lead of its four carriers' turns), into
    data whose :func:`turn_check` matches its sector's field lane header. Returns the lane
    data (atoms and check) per sector, sector 0 first; :func:`turn_packet` turns it into
    atoms.

    ``image`` is the luma plane ``frame`` was decoded from (``frame.pose`` maps into its
    pixel coordinates).
    """
    skip = tuple(bool(s) for s in skip)
    if len(skip) != SECTORS:
        raise ValueError("skip names the four sectors")
    results: List[Optional[LaneResult]] = [None] * SECTORS
    if not _readable(image):
        return tuple(results)
    reading = _decide_turns(image, frame, tuple(not s for s in skip), True)
    if reading is None:
        return tuple(results)
    for sector in range(SECTORS):
        span = reading.ranges[sector]
        header = reading.headers[sector]
        if span is None or header is None or skip[sector]:
            continue
        word = bytearray(TURN_WORD)
        confidence = [F64_MAX] * TURN_WORD
        for j, (position, margin) in enumerate(reading.decided[span[0] : span[1]]):
            word[j >> 2] |= _TURN_LABELS[position] << (6 - 2 * (j & 3))
            confidence[j >> 2] = f64_min(confidence[j >> 2], margin)
        results[sector] = _decode_turn_word(sector, header, bytes(word), confidence)
    return tuple(results)


@dataclass(frozen=True)
class ObservedTurns:
    """The turns a reader decided for a capture, for diagnostics."""

    #: The blur (Gaussian sigma in canvas units) that fits the carriers best.
    blur: float
    #: Every carrier of the decoded sectors: its cell, the turn position decided for it,
    #: and how far that position's score led the runner-up's.
    carriers: Tuple[Tuple[int, int, float], ...]


def observed_turns(image: Luma, frame: DecodedFrame) -> Optional[ObservedTurns]:
    """What :func:`read_turns` decides for every carrier of the sectors ``frame`` decoded,
    before any Reed-Solomon correction and whatever the blur; ``None`` when no decoded
    sector has carriers or the plane cannot be sampled."""
    if not _readable(image):
        return None
    reading = _decide_turns(image, frame, _EVERY_SECTOR, False)
    if reading is None:
        return None
    return ObservedTurns(
        blur=TURN_BLURS[reading.fitted],
        carriers=tuple(
            (cell, position, lead)
            for cell, (position, lead) in zip(reading.carriers, reading.decided)
        ),
    )
