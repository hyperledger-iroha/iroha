# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""From a camera luma plane to lane data.

The decoder locates the corner finders (four, or three with the fourth
inferred), derives a pose for each orientation hypothesis, ranks the
orientations by how well the ring gates line up (lane ``D`` must check out),
then reads the dots and the petal field. Every petal is one bit: its level,
normalised by the finder levels, is compared with a threshold computed for its
codeword and colour class (the petals inside the ``天`` are lighter than the
pink ones). Bytes the decoder is unsure about become Reed-Solomon erasures.

When some field lanes do not decode, the pose is searched for radial lens
distortion: a cheap camera's barrel distortion moves the petals by a third of
their pitch at the edge of a 480p frame, while the finders that fix the
homography sit far outside the field. Field lanes the finder levels leave
unread are read again from *local levels*, every cell rescaled by its
neighbourhood, so shadows, glare, banding and veiling light cancel.

A camera sees each shown frame two to four times. :func:`field_evidence` keeps
what one capture shows of the petal field, and :func:`merge_fields` reads the
field lanes from the average of several captures of one shown frame, which
recovers lanes that noise, display banding or hand shake cost each capture
alone. :func:`track_frame` follows a code from the previous frame's pose
without searching the whole image.

This is an operation-by-operation port of the Rust reference in IEEE double
precision, including the order of every floating-point sum, so it reads the
same lanes from the same pixels.
"""

from __future__ import annotations

import enum
import math
from dataclasses import dataclass, replace
from functools import reduce
from operator import add
from typing import Callable, Dict, List, Optional, Sequence, Tuple

from ._numeric import F64_MAX, NAN, f64_max, ieee_div, total_key
from .image import Luma, Pose
from .lanes import D_WORD, FIELD_LANES, FIELD_WORD, FrameCells, Lane, decode_lane_counted
from .layout import (
    BAND_COUNT,
    BAND_OFFSETS,
    DOT_RADIUS,
    FIELD_CELLS,
    FINDER_CENTERS,
    PETAL_RADIUS,
    RING_COUNT,
    SECTORS,
    TOTAL_SLOTS,
    cell_band_slot,
    cell_center,
    cell_light,
    data_slots,
    gate_slots,
    guard_slots,
    sector_cells,
    slot_center,
    split_slot,
)
from .locate import Finder, FinderSet, candidates, follow
from .rs import RsError
from .stream import AtomPacket, Beacon, DLane, StreamAssembler, parse_atom_lane, parse_d_lane

__all__ = [
    "DecodeOptions",
    "LaneResult",
    "DecodedFrame",
    "DecodeErrorKind",
    "DecodeError",
    "FieldEvidence",
    "decode_frame",
    "decode_frame_at",
    "track_frame",
    "observed_cells",
    "field_evidence",
    "merge_fields",
]

#: Distortion steps tried, relative to the starting pose, when a plain pose leaves field
#: lanes unread; the best is then refined by +-:data:`_DISTORTION_REFINE`.
_DISTORTION_STEPS = (-0.03, -0.06, -0.09, -0.12, -0.16, 0.03)
#: Final refinement step of the distortion search.
_DISTORTION_REFINE = 0.015
#: A colour class with fewer cells in a codeword than this takes the codeword's threshold.
_CLASS_MIN_CELLS = 16
#: Field cells within this distance of a cell are its neighbourhood for the local read.
_NEIGHBOURHOOD = 32.0
#: Cells averaged at each end of a sorted neighbourhood for its local dark and lit levels.
_NEIGHBOURHOOD_ENDS = 5
#: Field cells of a sector that carry the bits of its codeword.
_FIELD_BITS = FIELD_WORD * 8
_NOT_SKIPPED = (False,) * SECTORS
_NO_FIELDS: Tuple[Optional["LaneResult"], ...] = (None,) * SECTORS


@dataclass(frozen=True)
class DecodeOptions:
    """Decoder tuning."""

    #: Also try horizontally mirrored images (front-camera previews).
    try_mirrored: bool = True
    #: Largest image (in pixels) the decoder accepts; larger frames should be
    #: downscaled by the caller. Bounds memory and work on hostile input.
    max_pixels: int = 12_000_000


@dataclass(frozen=True)
class LaneResult:
    """A lane that passed its Reed-Solomon check."""

    #: Lane data bytes (header and atoms, or the beacon).
    data: bytes
    #: Byte positions the Reed-Solomon decoder rewrote: the erased bytes plus
    #: any errors it found among the others (a measure of how close the lane
    #: was to failing).
    corrected: int
    #: Bytes that were passed to the decoder as erasures.
    erasures: int


@dataclass(frozen=True)
class DecodedFrame:
    """Everything read from one camera frame."""

    #: Canvas-to-pixel pose that was used.
    pose: Pose
    #: Orientation: how many quarter turns the code is rotated.
    rotation: int
    #: Whether the image was mirrored.
    mirrored: bool
    #: Lane ``D`` result.
    d: Optional[LaneResult]
    #: Field lane results, sector 0 first.
    fields: Tuple[Optional[LaneResult], ...] = _NO_FIELDS
    #: The corner finder that was hidden (by a finger, a glare or the edge of
    #: the frame) and inferred from the other three, as its canonical index:
    #: 0 top-left, 1 top-right, 2 bottom-right, 3 bottom-left of the upright code.
    inferred_corner: Optional[int] = None

    def __post_init__(self) -> None:
        fields = tuple(self.fields)
        if len(fields) != SECTORS:
            raise ValueError("a frame has four field lanes")
        object.__setattr__(self, "fields", fields)

    @property
    def lanes_ok(self) -> int:
        """Number of lanes that decoded."""
        return (self.d is not None) + sum(1 for f in self.fields if f is not None)

    def lane(self, lane: Lane) -> Optional[LaneResult]:
        """The result of ``lane``, when it decoded."""
        if lane is Lane.D:
            return self.d
        return self.fields[FIELD_LANES.index(lane)]

    @property
    def lanes(self) -> str:
        """The lanes that decoded, as letters: ``D`` and the sector digits ``0``-``3``."""
        letters = "D" if self.d is not None else ""
        return letters + "".join(
            lane.letter for lane, result in zip(FIELD_LANES, self.fields) if result is not None
        )

    def d_lane(self) -> Optional[DLane]:
        """What lane ``D`` carried, when it decoded."""
        if self.d is None:
            return None
        return parse_d_lane(self.d.data)

    def beacon(self) -> Optional[Beacon]:
        """The beacon, when lane ``D`` decoded on a beacon frame."""
        lane = self.d_lane()
        return lane if isinstance(lane, Beacon) else None

    def atom_packets(self) -> List[AtomPacket]:
        """Atom packets from every lane that decoded."""
        packets = []
        for lane, result in zip(FIELD_LANES, self.fields):
            if result is not None:
                packet = parse_atom_lane(lane, result.data)
                if packet is not None:
                    packets.append(packet)
        d_lane = self.d_lane()
        if isinstance(d_lane, AtomPacket):
            packets.append(d_lane)
        return packets

    def feed(self, assembler: StreamAssembler) -> None:
        """Offer everything this frame carries to ``assembler``."""
        d_lane = self.d_lane()
        if d_lane is not None:
            assembler.push_d_lane(d_lane)
        for lane, result in zip(FIELD_LANES, self.fields):
            if result is not None:
                packet = parse_atom_lane(lane, result.data)
                if packet is not None:
                    assembler.push_atoms(packet)


class DecodeErrorKind(enum.Enum):
    """Why a frame could not be decoded at all."""

    #: The image is empty, smaller than 48 pixels on a side, or larger than
    #: :attr:`DecodeOptions.max_pixels`.
    UNSUPPORTED_IMAGE = "petal image has an unsupported size"
    #: No set of corner finders (four, or three forming a corner) was found.
    NO_FINDERS = "petal finders not found"
    #: Finders were found but no orientation produced a readable lane.
    NO_ORIENTATION = "no petal orientation produced a readable lane"


class DecodeError(Exception):
    """A frame could not be decoded; :attr:`kind` says why."""

    def __init__(self, kind: DecodeErrorKind) -> None:
        super().__init__(kind.value)
        self.kind = kind


Projector = Callable[[float, float], float]


def _projector(image: Luma, pose: Pose) -> Projector:
    """``image.sample(*pose.apply(x, y))`` as one fast closure."""
    m0, m1, m2, m3, m4, m5, m6, m7, m8 = pose.homography.m
    k = pose.distortion
    ox, oy = pose.center
    scale2 = pose.scale2
    data = image.data
    w = image.width
    wm1 = w - 1
    hm1 = image.height - 1
    fwm1 = float(wm1)
    fhm1 = float(hm1)

    def sample(px: float, py: float) -> float:
        fx = px - 0.5
        fy = py - 0.5
        if fx != fx or fy != fy:
            return NAN
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
        return top * (1.0 - ty) + bottom * ty

    if k == 0.0:

        def project(x: float, y: float) -> float:
            den = m6 * x + m7 * y + m8
            if den == 0.0:
                return sample(
                    ieee_div(m0 * x + m1 * y + m2, den), ieee_div(m3 * x + m4 * y + m5, den)
                )
            # inlined `sample`
            fx = (m0 * x + m1 * y + m2) / den - 0.5
            fy = (m3 * x + m4 * y + m5) / den - 0.5
            if fx != fx or fy != fy:
                return NAN
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
            return top * (1.0 - ty) + bottom * ty

        return project

    def project_distorted(x: float, y: float) -> float:
        den = m6 * x + m7 * y + m8
        if den == 0.0:
            ux = ieee_div(m0 * x + m1 * y + m2, den)
            uy = ieee_div(m3 * x + m4 * y + m5, den)
        else:
            ux = (m0 * x + m1 * y + m2) / den
            uy = (m3 * x + m4 * y + m5) / den
        dx = ux - ox
        dy = uy - oy
        factor = 1.0 + ieee_div(k * (dx * dx + dy * dy), scale2)
        return sample(ox + dx * factor, oy + dy * factor)

    return project_distorted


class _Reference:
    """Lit and dark levels measured at the four finders."""

    __slots__ = ("lit", "dark")

    def __init__(self, lit: Tuple[float, ...], dark: Tuple[float, ...]) -> None:
        self.lit = lit
        self.dark = dark

    def at(self, x: float, y: float) -> Tuple[float, float]:
        """Bilinear interpolation over the canvas of the four corner estimates."""
        u = x / 1024.0
        if u < 0.0:
            u = 0.0
        elif u > 1.0:
            u = 1.0
        v = y / 1024.0
        if v < 0.0:
            v = 0.0
        elif v > 1.0:
            v = 1.0
        lit = self.lit
        dark = self.dark
        top = lit[0] * (1.0 - u) + lit[1] * u
        bottom = lit[3] * (1.0 - u) + lit[2] * u
        lit_level = top * (1.0 - v) + bottom * v
        top = dark[0] * (1.0 - u) + dark[1] * u
        bottom = dark[3] * (1.0 - u) + dark[2] * u
        return lit_level, top * (1.0 - v) + bottom * v


def _dot_samples(project: Projector, x: float, y: float, spread: float) -> float:
    total = project(x + 0.0, y + 0.0)
    total += project(x + spread, y + 0.0)
    total += project(x + -spread, y + 0.0)
    total += project(x + 0.0, y + spread)
    total += project(x + 0.0, y + -spread)
    return total / 5.0


# the blossom is solid out to radius 24 around its centre
_FINDER_RING = tuple(
    (20.0 * math.cos(math.tau * k / 8.0), 20.0 * math.sin(math.tau * k / 8.0)) for k in range(8)
)


def _reference_levels(project: Projector, inferred: Optional[int] = None) -> Optional[_Reference]:
    """Light and dark levels at the four corners: the solid blossom core, and the black
    canvas 100 units inward of it.

    An ``inferred`` corner (canonical index) was not seen, so its levels are
    extrapolated from the other three by the parallelogram rule and kept within their
    range; they must differ by 12 like a seen corner's.
    """
    lit = [0.0] * 4
    dark = [0.0] * 4
    for i, (cx, cy) in enumerate(FINDER_CENTERS):
        if i == inferred:
            continue
        total = project(cx, cy)
        for ox, oy in _FINDER_RING:
            total += project(cx + ox, cy + oy)
        lit_level = total / 9.0
        sx = 1.0 if cx < 512.0 else -1.0
        sy = 1.0 if cy < 512.0 else -1.0
        a = _dot_samples(project, cx + sx * 100.0, cy, 5.0)
        b = _dot_samples(project, cx, cy + sy * 100.0, 5.0)
        dark_level = 0.5 * (a + b)
        contrast = lit_level - dark_level
        # also refuses NaN levels, which only a non-finite pose can produce
        if not math.isfinite(contrast) or contrast < 12.0:
            return None
        lit[i] = lit_level
        dark[i] = dark_level
    if inferred is not None:
        m = inferred
        n1 = (m + 1) % 4
        opposite = (m + 2) % 4
        n2 = (m + 3) % 4
        for levels in (lit, dark):
            low = min(levels[n1], levels[opposite], levels[n2])
            high = max(levels[n1], levels[opposite], levels[n2])
            value = levels[n1] + levels[n2] - levels[opposite]
            if value < low:
                value = low
            elif value > high:
                value = high
            levels[m] = value
        # uneven light can push the estimates past each other; an inferred corner
        # needs the same contrast as a seen one
        if lit[m] - dark[m] < 12.0:
            return None
    return _Reference(tuple(lit), tuple(dark))


_CANONICAL = tuple((float(x), float(y)) for x, y in FINDER_CENTERS)


def _fit_pose(image: Luma, dst: Sequence[Tuple[float, float]], distortion: float) -> Optional[Pose]:
    """The pose with lens distortion ``distortion`` through the four finder centres ``dst``
    (image pixels, in canonical corner order)."""
    return Pose.fit(_CANONICAL, dst, distortion, image.width, image.height)


def _canonical_corner(index: int, rotation: int, mirrored: bool) -> int:
    """Canonical index of the corner at index ``index`` of a finder quad under one
    orientation hypothesis."""
    if mirrored:
        return (rotation + 4 - index) % 4
    return (index + 4 - rotation) % 4


_SLOT_POINTS = tuple(slot_center(*split_slot(flat)) for flat in range(TOTAL_SLOTS))
_GATES = gate_slots()
_GUARDS = guard_slots()
_GATES_BY_RING = tuple(
    tuple(s for s in _GATES if split_slot(s)[0] == ring) for ring in range(RING_COUNT)
)
_GUARDS_BY_RING = tuple(
    tuple(s for s in _GUARDS if split_slot(s)[0] == ring) for ring in range(RING_COUNT)
)
_DATA_SLOTS = data_slots()
_DATA_RINGS = tuple(split_slot(slot)[0] for slot in _DATA_SLOTS)
#: Sample spread of a ring dot, in canvas units.
_DOT_SPREAD = 0.32 * DOT_RADIUS
#: Sample spread of a petal, in canvas units.
_PETAL_SPREAD = 0.4 * PETAL_RADIUS

#: The five sample points of every ring slot, in flat slot order (``_dot_samples`` with
#: the dot spread, coordinates computed exactly as there).
_RING_SAMPLES = tuple(
    (
        x + 0.0,
        y + 0.0,
        x + _DOT_SPREAD,
        y + 0.0,
        x + -_DOT_SPREAD,
        y + 0.0,
        x + 0.0,
        y + _DOT_SPREAD,
        x + 0.0,
        y + -_DOT_SPREAD,
    )
    for x, y in _SLOT_POINTS
)


def _ring_brightness(image: Luma, corners: Sequence[Tuple[float, float]]) -> Optional[float]:
    """The brightness summed over all ring slots under the plain pose that maps the
    canonical corners onto ``corners`` (in quad order).

    The slots form the same set of points under every quarter turn and mirror of the
    canvas (88, 100 and 108 are multiples of four), so the value does not depend on the
    orientation.
    """
    pose = _fit_pose(image, corners, 0.0)
    if pose is None:
        return None
    project = _projector(image, pose)
    # every sample is at least +0.0 (or NaN), so starting the sum at 0.0, as the
    # reference's `dot_samples` and slot sum do, adds nothing
    total = 0.0
    for ax, ay, bx, by, cx, cy, dx, dy, ex, ey in _RING_SAMPLES:
        samples = project(ax, ay)
        samples += project(bx, by)
        samples += project(cx, cy)
        samples += project(dx, dy)
        samples += project(ex, ey)
        total += samples / 5.0
    return total


def _refine_inferred_corner(
    image: Luma, corners: Sequence[Finder], inferred: int
) -> Tuple[Finder, ...]:
    """Moves an inferred corner to where the three dotted rings line up best.

    A 13 x 13 search in steps of 2 % of the mean leg around the parallelogram estimate,
    then a 9 x 9 search in steps of 0.5 % around the best point. The rings fix the
    geometry only; the orientation is decided afterwards by the gates and the lanes.
    """
    points = [(f.x, f.y) for f in corners]
    sx, sy = points[inferred]

    def distance(other: int) -> float:
        ex = points[other][0] - sx
        ey = points[other][1] - sy
        return math.sqrt(ex * ex + ey * ey)

    leg = 0.5 * (distance((inferred + 1) % 4) + distance((inferred + 3) % 4))
    best = [-F64_MAX, (sx, sy)]

    def search(centre: Tuple[float, float], step: float, reach: int) -> None:
        cx, cy = centre
        for dy in range(-reach, reach + 1):
            y = cy + dy * step
            for dx in range(-reach, reach + 1):
                candidate = (cx + dx * step, y)
                points[inferred] = candidate
                brightness = _ring_brightness(image, points)
                if brightness is not None and brightness > best[0]:
                    best[0] = brightness
                    best[1] = candidate

    search((sx, sy), 0.02 * leg, 6)
    search(best[1], 0.005 * leg, 4)
    refined = list(corners)
    bx, by = best[1]
    refined[inferred] = Finder(bx, by, corners[inferred].size)
    return tuple(refined)


def _normalised_dot(project: Projector, reference: _Reference, flat: int) -> float:
    x, y = _SLOT_POINTS[flat]
    lit, dark = reference.at(x, y)
    return ieee_div(_dot_samples(project, x, y, _DOT_SPREAD) - dark, lit - dark)


def _sequential_sum(values: Sequence[float]) -> float:
    """Left-to-right float sum, as Rust's ``Iterator::sum`` computes it.

    (Python's built-in ``sum`` of floats is compensated from 3.12 on, so it is never
    used where the reference sums.)
    """
    return reduce(add, values) if values else 0.0


def _gate_score(project: Projector, reference: _Reference) -> float:
    gates = [_normalised_dot(project, reference, s) for s in _GATES]
    guards = [_normalised_dot(project, reference, s) for s in _GUARDS]
    return _sequential_sum(gates) / len(gates) - _sequential_sum(guards) / len(guards)


def _read_dots(project: Projector, reference: _Reference) -> Tuple[bytearray, List[float]]:
    """Read lane ``D``: the transmitted bytes and per-byte confidence."""
    # per-ring thresholds from the gates (lit) and guards (dark)
    thresholds = [0.5] * RING_COUNT
    for ring in range(RING_COUNT):
        lit = [_normalised_dot(project, reference, s) for s in _GATES_BY_RING[ring]]
        dark = [_normalised_dot(project, reference, s) for s in _GUARDS_BY_RING[ring]]
        if lit and dark:
            level_lit = _sequential_sum(lit) / len(lit)
            level_dark = _sequential_sum(dark) / len(dark)
            if level_lit - level_dark > 0.2:
                thresholds[ring] = 0.5 * (level_lit + level_dark)
    word = bytearray(D_WORD)
    confidence = [F64_MAX] * D_WORD
    for bit, slot in enumerate(_DATA_SLOTS):
        value = _normalised_dot(project, reference, slot)
        threshold = thresholds[_DATA_RINGS[bit]]
        if value > threshold:
            word[bit >> 3] |= 1 << (7 - (bit & 7))
        margin = abs(value - threshold)
        if margin < confidence[bit >> 3]:
            confidence[bit >> 3] = margin
    return word, confidence


def _read_lane_d(project: Projector, reference: _Reference) -> Optional[LaneResult]:
    """Lane ``D`` under one pose."""
    word, confidence = _read_dots(project, reference)
    return _decode_with_erasures(Lane.D, word, confidence)


def _decode_with_erasures(
    lane: Lane, word: bytes, confidence: Sequence[float]
) -> Optional[LaneResult]:
    """Try Reed-Solomon with growing numbers of erasures, least confident first.

    The schedule erases 0, 1/8, 1/4, 1/3 and 1/2 of the parity bytes (integer division:
    lane ``D`` 0, 1, 2, 3, 5; a field lane 0, 2, 5, 7, 11), the least confident bytes
    first with ties in position order. Lane ``D`` has only 11 parity bytes, and a further
    step leaves so few spare ones that it accepts wrong codewords (at 7 erasures about
    0.4 % of random words).
    """
    nsym = lane.parity_len
    order = sorted(range(len(word)), key=lambda i: total_key(confidence[i]))
    schedule: List[int] = []
    for count in (0, nsym // 8, nsym // 4, nsym // 3, nsym // 2):
        if not schedule or schedule[-1] != count:
            schedule.append(count)
    for erasures in schedule:
        positions = order[:erasures]
        trial = bytearray(word)
        # zero the erased bytes so stale values cannot leak through
        for p in positions:
            trial[p] = 0
        try:
            data, corrected = decode_lane_counted(lane, trial, positions)
        except RsError:
            continue
        return LaneResult(data=data, corrected=corrected, erasures=len(positions))
    return None


def _two_means(values: Sequence[float]) -> float:
    """The two-means threshold of a set of values: start at their mean, then move to the
    midpoint of the means of the values above and at or below the threshold, at most
    eight times, stopping when it moves less than 1e-4 or one side is empty. Sums run in
    the order given."""
    count = len(values)
    threshold = reduce(add, values) / count
    for _ in range(8):
        high = [v for v in values if v > threshold]
        highs = len(high)
        lows = count - highs
        if lows == 0 or highs == 0:
            break
        low = [v for v in values if not v > threshold]
        following = 0.5 * (reduce(add, low, 0.0) / lows + reduce(add, high, 0.0) / highs)
        moved = abs(following - threshold)
        threshold = following
        if moved < 1e-4:
            break
    return threshold


#: The bit-carrying cells of every sector, in bit order.
_SECTOR_BITS = tuple(sector_cells(sector)[:_FIELD_BITS] for sector in range(SECTORS))
#: Whether each bit-carrying cell of every sector lies in the ``天`` (light class).
_SECTOR_LIGHT = tuple(tuple(cell_light(c) for c in cells) for cells in _SECTOR_BITS)
_CELL_CENTERS = tuple(cell_center(cell) for cell in range(FIELD_CELLS))


def _clamp_unit(value: float) -> float:
    if value < 0.0:
        return 0.0
    if value > 1.0:
        return 1.0
    return value


#: The five sample points of every field cell (``_dot_samples`` with the petal spread,
#: coordinates computed exactly as there).
_FIELD_SAMPLES = tuple(
    (
        x + 0.0,
        y + 0.0,
        x + _PETAL_SPREAD,
        y + 0.0,
        x + -_PETAL_SPREAD,
        y + 0.0,
        x + 0.0,
        y + _PETAL_SPREAD,
        x + 0.0,
        y + -_PETAL_SPREAD,
    )
    for x, y in _CELL_CENTERS
)
#: ``(1 - u, u, 1 - v, v)`` of every field cell for :meth:`_Reference.at`.
_FIELD_WEIGHTS = tuple(
    (1.0 - u, u, 1.0 - v, v)
    for u, v in ((_clamp_unit(x / 1024.0), _clamp_unit(y / 1024.0)) for x, y in _CELL_CENTERS)
)


def _field_levels(project: Projector, reference: _Reference) -> List[float]:
    """The level of every field cell under one pose, normalised by the finder levels."""
    l0, l1, l2, l3 = reference.lit
    d0, d1, d2, d3 = reference.dark
    out = []
    append = out.append
    for (ax, ay, bx, by, cx, cy, dx, dy, ex, ey), (iu, u, iv, v) in zip(
        _FIELD_SAMPLES, _FIELD_WEIGHTS
    ):
        lit = (l0 * iu + l1 * u) * iv + (l3 * iu + l2 * u) * v
        dark = (d0 * iu + d1 * u) * iv + (d3 * iu + d2 * u) * v
        samples = project(ax, ay)
        samples += project(bx, by)
        samples += project(cx, cy)
        samples += project(dx, dy)
        samples += project(ex, ey)
        scale = lit - dark
        if scale != 0.0:
            append((samples / 5.0 - dark) / scale)
        else:
            append(ieee_div(samples / 5.0 - dark, scale))
    return out


_NEIGHBOURS: Optional[Tuple[Tuple[int, ...], ...]] = None


def _neighbourhoods() -> Tuple[Tuple[int, ...], ...]:
    """The neighbourhood of every field cell: the cells (itself included) whose centres
    lie within :data:`_NEIGHBOURHOOD` units, in index order. Bands are 11 units apart, so
    only the three bands on either side can hold neighbours. Built on first use."""
    global _NEIGHBOURS
    if _NEIGHBOURS is None:
        limit = _NEIGHBOURHOOD * _NEIGHBOURHOOD
        # Cells are bucketed on a grid wider than the neighbourhood radius, so every
        # neighbour lies in one of the 3 x 3 buckets around a cell; the distance test and
        # the band range below are the reference's.
        pitch = _NEIGHBOURHOOD + 1.0
        buckets: Dict[Tuple[int, int], List[int]] = {}
        for cell, (x, y) in enumerate(_CELL_CENTERS):
            buckets.setdefault((int(x // pitch), int(y // pitch)), []).append(cell)
        table = []
        for cell, (x, y) in enumerate(_CELL_CENTERS):
            band, _ = cell_band_slot(cell)
            first = BAND_OFFSETS[max(band - 3, 0)]
            last = BAND_OFFSETS[min(band + 4, BAND_COUNT)]
            bx = int(x // pitch)
            by = int(y // pitch)
            around = []
            for gx in (bx - 1, bx, bx + 1):
                for gy in (by - 1, by, by + 1):
                    for other in buckets.get((gx, gy), ()):
                        if first <= other < last:
                            ox, oy = _CELL_CENTERS[other]
                            ex = ox - x
                            ey = oy - y
                            if ex * ex + ey * ey <= limit:
                                around.append(other)
            around.sort()
            table.append(tuple(around))
        _NEIGHBOURS = tuple(table)
    return _NEIGHBOURS


def _local_levels(levels: Sequence[float]) -> List[float]:
    """Cell levels rescaled by their neighbourhood: the mean of the lowest and of the
    highest :data:`_NEIGHBOURHOOD_ENDS` levels around a cell become 0 and 1 (a span below
    0.05 counts as 0.05).

    About half the petals around any cell are lit, so shadows, glare, banding and veiling
    light, which the finder levels cannot describe, cancel.
    """
    # the plain order is Rust's `total_cmp` order unless a NaN is present (zeros of
    # either sign tie, which no sum below can tell apart)
    ordered = sorted if all(v == v for v in levels) else _sorted_total
    out = []
    append = out.append
    for cell, around in enumerate(_neighbourhoods()):
        values = ordered([levels[o] for o in around])
        count = len(values)
        ends = min(_NEIGHBOURHOOD_ENDS, count // 2)
        if ends < 1:
            ends = 1
        low = reduce(add, values[:ends]) / ends
        high = reduce(add, values[count - ends :]) / ends
        span = high - low
        if not span > 0.05:  # `f64::max(0.05)`: a NaN span yields 0.05
            span = 0.05
        append((levels[cell] - low) / span)
    return out


def _sorted_total(values: Sequence[float]) -> List[float]:
    """``values`` ascending in Rust's ``f64::total_cmp`` order."""
    return sorted(values, key=total_key)


def _read_word(
    levels: Sequence[float], sector: int, margins: float
) -> Tuple[bytearray, List[float], float]:
    """The transmitted word of one sector read from cell levels, each byte's confidence
    (the smallest margin of its bits), and ``margins`` with every bit's margin from its
    threshold added in bit order."""
    cells = _SECTOR_BITS[sector]
    light = _SECTOR_LIGHT[sector]
    values = [levels[c] for c in cells]
    every = _two_means(values)
    # the light (`天`) and pink petals are thresholded apart: their lit levels differ
    members = [v for v, is_light in zip(values, light) if is_light]
    light_threshold = every if len(members) < _CLASS_MIN_CELLS else _two_means(members)
    members = [v for v, is_light in zip(values, light) if not is_light]
    pink_threshold = every if len(members) < _CLASS_MIN_CELLS else _two_means(members)
    word = bytearray(FIELD_WORD)
    confidence = [F64_MAX] * FIELD_WORD
    for bit, (value, is_light) in enumerate(zip(values, light)):
        threshold = light_threshold if is_light else pink_threshold
        if value > threshold:
            word[bit >> 3] |= 1 << (7 - (bit & 7))
        margin = abs(value - threshold)
        margins += margin
        if margin < confidence[bit >> 3]:
            confidence[bit >> 3] = margin
    return word, confidence, margins


def _read_sectors(
    levels: Sequence[float], skip: Sequence[bool]
) -> Tuple[List[Optional[LaneResult]], float]:
    """One read of the field lanes from cell levels with per-codeword, per-colour
    thresholds, skipping the sectors in ``skip``, and how far the cells read sit from
    their thresholds on average (one running sum over the sectors read)."""
    separation = 0.0
    counted = 0
    results: List[Optional[LaneResult]] = [None] * SECTORS
    for sector in range(SECTORS):
        if skip[sector]:
            continue
        word, confidence, separation = _read_word(levels, sector, separation)
        counted += _FIELD_BITS
        results[sector] = _decode_with_erasures(FIELD_LANES[sector], word, confidence)
    return results, separation / max(counted, 1)


def _read_fields(
    levels: Sequence[float], skip: Sequence[bool]
) -> Tuple[List[Optional[LaneResult]], float]:
    """The field lanes read from normalised cell levels, skipping the sectors in ``skip``,
    and the separation of that first read. Sectors the levels leave unread are read again
    from their :func:`_local_levels`."""
    results, separation = _read_sectors(levels, skip)
    unread = [not skip[s] and results[s] is None for s in range(SECTORS)]
    if any(unread):
        retry, _ = _read_sectors(_local_levels(levels), [not u for u in unread])
        for sector in range(SECTORS):
            if results[sector] is None:
                results[sector] = retry[sector]
    return results, separation


class _FieldRead:
    """A read of the field under one pose."""

    __slots__ = ("pose", "project", "reference", "levels", "fields", "decoded", "separation")

    def __init__(
        self,
        pose: Pose,
        project: Projector,
        reference: _Reference,
        levels: List[float],
        fields: List[Optional[LaneResult]],
        separation: float,
    ) -> None:
        self.pose = pose
        self.project = project
        self.reference = reference
        self.levels = levels
        self.fields = fields
        self.decoded = sum(1 for f in fields if f is not None)
        self.separation = separation

    def better_than(self, other: "_FieldRead") -> bool:
        """More lanes, then a clearer separation."""
        return (self.decoded, self.separation) > (other.decoded, other.separation)

    def finish(self) -> "_FieldRead":
        """Reads the sectors this pose left unread again from their local levels."""
        if self.decoded < SECTORS:
            unread = [f is None for f in self.fields]
            retry, _ = _read_sectors(_local_levels(self.levels), [not u for u in unread])
            for sector in range(SECTORS):
                if self.fields[sector] is None:
                    self.fields[sector] = retry[sector]
            self.decoded = sum(1 for f in self.fields if f is not None)
        return self


def _read_field_at(
    image: Luma, dst: Sequence[Tuple[float, float]], distortion: float, inferred: Optional[int]
) -> Optional[_FieldRead]:
    """The field under the pose with distortion ``distortion`` through ``dst`` (no local
    read)."""
    pose = _fit_pose(image, dst, distortion)
    if pose is None:
        return None
    project = _projector(image, pose)
    reference = _reference_levels(project, inferred)
    if reference is None:
        return None
    levels = _field_levels(project, reference)
    fields, separation = _read_sectors(levels, _NOT_SKIPPED)
    return _FieldRead(pose, project, reference, levels, fields, separation)


def _read_field_searching(
    image: Luma,
    dst: Sequence[Tuple[float, float]],
    start: float,
    inferred: Optional[int],
    first: Optional[_FieldRead] = None,
) -> Optional[_FieldRead]:
    """Reads the field through the finder centres ``dst``, starting from lens distortion
    ``start``, and searches the distortion while some field lane stays unread: ``start``
    plus each of :data:`_DISTORTION_STEPS` in turn, then the best +-
    :data:`_DISTORTION_REFINE`. The pose that decodes the most lanes wins (ties: the
    clearer separation); the search stops once every lane decodes. Lanes the winning pose
    still leaves unread are read again from its local levels.

    ``first``, when given, is the read at ``start`` the caller already made (the read is
    deterministic, so it is not repeated).
    """
    best = first if first is not None else _read_field_at(image, dst, start, inferred)
    if best is None:
        return None
    if best.decoded == SECTORS:
        return best
    for step in _DISTORTION_STEPS:
        read = _read_field_at(image, dst, start + step, inferred)
        if read is not None and read.better_than(best):
            best = read
            if best.decoded == SECTORS:
                return best
    centre = best.pose.distortion
    for k in (centre - _DISTORTION_REFINE, centre + _DISTORTION_REFINE):
        read = _read_field_at(image, dst, k, inferred)
        if read is not None and read.better_than(best):
            best = read
    return best.finish()


def _hypotheses(
    finders: Sequence[Finder], try_mirrored: bool
) -> List[Tuple[int, bool, List[Tuple[float, float]]]]:
    """The four orientation hypotheses (and four mirrored ones): quarter turns, mirrored,
    and the finder centres in canonical corner order."""
    out = []
    for mirrored in (False, True):
        if mirrored and not try_mirrored:
            continue
        for rotation in range(4):
            dst = []
            for i in range(4):
                q = finders[(rotation + 4 - i) % 4] if mirrored else finders[(i + rotation) % 4]
                dst.append((q.x, q.y))
            out.append((rotation, mirrored, dst))
    return out


def _supported(image: Luma, options: DecodeOptions) -> bool:
    return (
        image.width >= 48
        and image.height >= 48
        and image.width * image.height <= options.max_pixels
        and len(image.data) == image.width * image.height
    )


def _decode(image: Luma, options: DecodeOptions) -> DecodedFrame:
    if not _supported(image, options):
        raise DecodeError(DecodeErrorKind.UNSUPPORTED_IMAGE)
    located = False
    for finder_set in candidates(image):
        located = True
        frame = _decode_candidate(image, options, finder_set)
        if frame is not None:
            return frame
    raise DecodeError(DecodeErrorKind.NO_ORIENTATION if located else DecodeErrorKind.NO_FINDERS)


def _decode_candidate(
    image: Luma, options: DecodeOptions, finder_set: FinderSet
) -> Optional[DecodedFrame]:
    """One candidate finder set: orientation hypotheses ranked by gate score; lane ``D``
    under the best three whose gate score is at least 0.2, then the field lanes under the
    best four."""
    index = finder_set.inferred
    corners = finder_set.corners
    if index is not None:
        corners = _refine_inferred_corner(image, corners, index)
    scored = []
    for rotation, mirrored, dst in _hypotheses(corners, options.try_mirrored):
        inferred = None if index is None else _canonical_corner(index, rotation, mirrored)
        pose = _fit_pose(image, dst, 0.0)
        if pose is None:
            continue
        project = _projector(image, pose)
        reference = _reference_levels(project, inferred)
        if reference is None:
            continue
        gate = _gate_score(project, reference)
        scored.append((gate, rotation, mirrored, dst, pose, project, reference, inferred))
    # stable sort, highest gate score first (Rust: `b.0.total_cmp(&a.0)`)
    scored.sort(key=lambda entry: total_key(entry[0]), reverse=True)
    # 1. the ring beacon is the cheapest and strongest orientation check
    for gate, rotation, mirrored, dst, pose, project, reference, inferred in scored[:3]:
        if gate < 0.2:
            continue
        d = _read_lane_d(project, reference)
        if d is not None:
            read = _read_field_searching(image, dst, 0.0, inferred)
            return DecodedFrame(
                pose=pose if read is None else read.pose,
                rotation=rotation,
                mirrored=mirrored,
                d=d,
                fields=_NO_FIELDS if read is None else tuple(read.fields),
                inferred_corner=inferred,
            )
    # 2. fall back to the field lanes under the most promising orientations: a wrong
    #    orientation reads permuted petals, which no field codeword accepts. Strong lens
    #    distortion can hide both lane D and the field from a plain pose, so the distortion
    #    is searched under the best orientation whose gates line up even when its plain
    #    pose reads nothing.
    for position, (gate, rotation, mirrored, dst, _, _, _, inferred) in enumerate(scored[:4]):
        plain = _read_field_at(image, dst, 0.0, inferred)
        if plain is None:
            continue
        confident = position == 0 and gate >= 0.2
        # the local read costs a sort per cell: only the best orientation gets it for free
        if plain.decoded == SECTORS:
            read = plain
        elif plain.decoded == 0 and not confident:
            read = plain.finish() if position == 0 else plain
        else:
            searched = _read_field_searching(image, dst, 0.0, inferred, plain)
            read = searched if searched is not None else plain.finish()
        if read.decoded == 0:
            continue
        return DecodedFrame(
            pose=read.pose,
            rotation=rotation,
            mirrored=mirrored,
            d=_read_lane_d(read.project, read.reference),
            fields=tuple(read.fields),
            inferred_corner=inferred,
        )
    return None


def _track(image: Luma, previous: DecodedFrame, options: DecodeOptions) -> Optional[DecodedFrame]:
    corner = previous.inferred_corner
    # a frame built by the caller may name a corner that does not exist
    if not _supported(image, options) or (corner is not None and not 0 <= corner <= 3):
        return None
    apply = previous.pose.apply
    expected = []
    for cx, cy in FINDER_CENTERS:
        x, y = apply(cx, cy)
        ax, ay = apply(cx - 60.0, cy)
        bx, by = apply(cx + 60.0, cy)
        across = math.sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by))
        ax, ay = apply(cx, cy - 60.0)
        bx, by = apply(cx, cy + 60.0)
        down = math.sqrt((ax - bx) * (ax - bx) + (ay - by) * (ay - by))
        expected.append(Finder(x, y, f64_max(across, down)))
    short = float(min(image.width, image.height))
    isfinite = math.isfinite
    for f in expected:
        if not (isfinite(f.x) and isfinite(f.y) and isfinite(f.size)) or f.size > short:
            return None
    previously_inferred = previous.inferred_corner
    found: List[Optional[Finder]] = [
        None if i == previously_inferred else follow(image, e) for i, e in enumerate(expected)
    ]
    # the mean movement of the corners that were followed predicts the others
    moved = [(f.x - e.x, f.y - e.y) for f, e in zip(found, expected) if f is not None]
    if len(moved) < 3:
        return None
    shift_x = _sequential_sum([m[0] for m in moved]) / len(moved)
    shift_y = _sequential_sum([m[1] for m in moved]) / len(moved)

    def predicted(i: int) -> Finder:
        e = expected[i]
        return Finder(e.x + shift_x, e.y + shift_y, e.size)

    # a corner that was hidden is seen again only when its blossom is found right where the
    # others say it is (a bright thumb beside it must not count)
    if previously_inferred is not None:
        at = predicted(previously_inferred)
        again = follow(image, at)
        if again is not None:
            ex = again.x - at.x
            ey = again.y - at.y
            if not math.sqrt(ex * ex + ey * ey) <= 0.25 * at.size:
                again = None
        found[previously_inferred] = again
    lost = [i for i in range(4) if found[i] is None]
    inferred: Optional[int] = None
    if len(lost) == 1:
        inferred = lost[0]
        found[inferred] = predicted(inferred)
    elif lost:
        return None
    corners: Tuple[Finder, ...] = tuple(f for f in found if f is not None)
    if inferred is not None:
        corners = _refine_inferred_corner(image, corners, inferred)
    points = [(f.x, f.y) for f in corners]
    read = _read_field_searching(image, points, previous.pose.distortion, inferred)
    if read is None:
        return None
    d = _read_lane_d(read.project, read.reference)
    if d is None and read.decoded == 0:
        return None
    return DecodedFrame(
        pose=read.pose,
        rotation=previous.rotation,
        mirrored=previous.mirrored,
        d=d,
        fields=tuple(read.fields),
        inferred_corner=inferred,
    )


def _downscale_factor(image: Luma, max_side: Optional[int]) -> int:
    if max_side is None:
        return 1
    if max_side < 48:
        raise ValueError("max_side must be at least 48 pixels")
    side = max(image.width, image.height)
    return max(1, -(-side // max_side))


def decode_frame(
    image: Luma, options: Optional[DecodeOptions] = None, *, max_side: Optional[int] = None
) -> DecodedFrame:
    """Decode one camera frame.

    Tries the finder candidates of :func:`iroha_petal.locate.candidates` in order
    and returns the first that reads. For each, the orientation hypotheses (four
    quarter turns, optionally mirrored) are ranked by the ring gates; lane ``D``
    is tried under the best three whose gate score is at least 0.2 (the field is
    then read under the winner, with a lens-distortion search when a field lane
    stays unread), then the field lanes under the best four. A candidate with a
    hidden corner blossom reports it in :attr:`DecodedFrame.inferred_corner`.

    Raises :class:`DecodeError` with :attr:`DecodeErrorKind.UNSUPPORTED_IMAGE`
    for unusable images, :attr:`DecodeErrorKind.NO_FINDERS` when no code is
    visible and :attr:`DecodeErrorKind.NO_ORIENTATION` when no orientation
    yields a readable lane.

    ``max_side`` (off by default) first box-filters images whose longer side
    exceeds it by the smallest integer factor that fits; the returned pose
    still maps into the original image's pixel coordinates.
    """
    if options is None:
        options = DecodeOptions()
    factor = _downscale_factor(image, max_side)
    if factor == 1:
        return _decode(image, options)
    frame = _decode(image.downscaled(factor), options)
    return replace(frame, pose=frame.pose.scaled(factor))


def track_frame(
    image: Luma, previous: DecodedFrame, options: Optional[DecodeOptions] = None
) -> Optional[DecodedFrame]:
    """Follow a code from the previous frame that decoded, without searching the whole
    image for finders (the most expensive part of :func:`decode_frame`).

    Each corner finder seen in the previous frame is re-found near where the previous
    pose puts it (see :func:`iroha_petal.locate.follow`); the mean movement of those
    predicts the rest. A corner inferred in the previous frame counts as seen again only
    when its blossom is re-found within a quarter diameter of that prediction (so a thumb
    beside it does not count). When exactly one corner is missing it is placed at the
    prediction and refined against the rings like an inferred corner. The orientation is
    kept from the previous frame and its lens distortion is the starting point of the
    distortion search. Returns ``None`` when the image is unusable, the previous pose is
    broken, two corners are lost or no lane decodes; the caller then runs
    :func:`decode_frame`.

    ``previous.pose`` must map into the pixel coordinates of ``image``.
    """
    if options is None:
        options = DecodeOptions()
    return _track(image, previous, options)


def decode_frame_at(
    image: Luma, pose: Pose, options: Optional[DecodeOptions] = None
) -> Optional[DecodedFrame]:
    """Read all lanes with a known canvas-to-pixel pose (no finder search, no distortion
    search).

    Returns ``None`` when the image is unusable (see
    :attr:`DecodeErrorKind.UNSUPPORTED_IMAGE`) or the finder reference levels
    are too weak. The reference levels come from all four corner finders, so a
    frame whose corner blossom is hidden is refused here; :func:`decode_frame`
    and :func:`track_frame` read such frames by inferring that corner. Used by
    refinement passes and by qualification tooling with a ground-truth pose.
    """
    if options is None:
        options = DecodeOptions()
    if not _supported(image, options):
        return None
    project = _projector(image, pose)
    reference = _reference_levels(project)
    if reference is None:
        return None
    fields, _ = _read_fields(_field_levels(project, reference), _NOT_SKIPPED)
    return DecodedFrame(
        pose=pose,
        rotation=0,
        mirrored=False,
        d=_read_lane_d(project, reference),
        fields=tuple(fields),
    )


class FieldEvidence:
    """What one capture shows of a frame's petal field: the level of every field cell
    sampled under the capture's decoded pose and normalised by that capture's finder
    levels. Captures of one shown frame merge with :func:`merge_fields`."""

    __slots__ = ("levels",)

    def __init__(self, levels: Sequence[float]) -> None:
        levels = tuple(float(v) for v in levels)
        if len(levels) != FIELD_CELLS:
            raise ValueError("field evidence holds one level per field cell")
        #: The normalised level of every field cell.
        self.levels: Tuple[float, ...] = levels

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, FieldEvidence):
            return NotImplemented
        return self.levels == other.levels

    def __repr__(self) -> str:
        return f"FieldEvidence({len(self.levels)} levels)"


def field_evidence(
    image: Luma, frame: DecodedFrame, options: Optional[DecodeOptions] = None
) -> Optional[FieldEvidence]:
    """The field evidence of ``image`` under the pose ``frame`` was decoded with.

    Returns ``None`` when the image is unusable (see
    :attr:`DecodeErrorKind.UNSUPPORTED_IMAGE`), the frame names a corner that does not
    exist, or the finder levels under that pose are too weak.
    """
    if options is None:
        options = DecodeOptions()
    corner = frame.inferred_corner
    # a frame built by the caller may name a corner that does not exist
    if not _supported(image, options) or (corner is not None and not 0 <= corner <= 3):
        return None
    project = _projector(image, frame.pose)
    reference = _reference_levels(project, corner)
    if reference is None:
        return None
    return FieldEvidence(_field_levels(project, reference))


def merge_fields(
    evidence: Sequence[FieldEvidence], skip: Sequence[bool] = _NOT_SKIPPED
) -> Tuple[Optional[LaneResult], ...]:
    """The field lanes from several captures of one shown frame: the cell-by-cell mean of
    their levels (summed in capture order, then divided by the number of captures), read
    as one capture's levels are, local read included.

    Averaging cancels the sensor noise, banding and shake that differ between captures;
    one evidence reads exactly the field lanes :func:`decode_frame` read from that
    capture's pose. The caller checks that every evidence comes from captures of the same
    shown frame: a lane read here names its frame in its own header. Sectors in ``skip``
    are not read.
    """
    if not evidence:
        return _NO_FIELDS
    if len(skip) != SECTORS:
        raise ValueError("skip names the four sectors")
    mean = [0.0] * FIELD_CELLS
    for capture in evidence:
        mean = list(map(add, mean, capture.levels))
    count = float(len(evidence))
    mean = [value / count for value in mean]
    fields, _ = _read_fields(mean, skip)
    return tuple(fields)


def observed_cells(
    image: Luma, frame: DecodedFrame, options: Optional[DecodeOptions] = None
) -> Optional[FrameCells]:
    """Build the cells a decoder believes it saw under ``frame``'s pose, for diagnostics:
    every codeword thresholded as the first read of :func:`decode_frame` thresholds it
    (no local read). The petals are left unturned
    (:func:`~iroha_petal.turns.observed_turns` reports the turns)."""
    if options is None:
        options = DecodeOptions()
    evidence = field_evidence(image, frame, options)
    if evidence is None:
        return None
    project = _projector(image, frame.pose)
    reference = _reference_levels(project, frame.inferred_corner)
    if reference is None:
        return None
    d, _ = _read_dots(project, reference)
    words = [bytes(_read_word(evidence.levels, sector, 0.0)[0]) for sector in range(SECTORS)]
    return FrameCells.from_words(bytes(d), words)
