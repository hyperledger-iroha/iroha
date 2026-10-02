# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""From a camera luma plane to lane data.

The decoder locates the four finders, derives a homography for each
orientation hypothesis, picks the orientation whose ring gates line up (and
whose lane ``D`` codeword checks out), then reads the tiles and dots. Every
tile is classified *jointly*: the 8x8 sample patch is compared against the 32
hypotheses (polarity x glyph) and the best match wins, so the katakana and the
light/dark bit help each other. Cells the decoder is unsure about become
Reed-Solomon erasures.

The tile *level read* judges every patch against the light and dark levels
measured at the finders. When it leaves lane ``P`` or ``K`` unreadable, the
*normalised read* is tried: it rescales each patch (and each template) by its
own contrast, so over-exposure, veiling light, glare, shadows and gradients
cancel out.

This is an operation-by-operation port of the Rust reference in IEEE double
precision, including the order of every floating-point sum, so it reads the
same lanes from the same pixels.
"""

from __future__ import annotations

import enum
import math
from dataclasses import dataclass
from functools import reduce
from operator import add, mul, sub
from typing import Callable, Dict, List, Optional, Sequence, Tuple

from ._numeric import F64_MAX, NAN, ieee_div, total_key
from .glyphs import GLYPH_COUNT, TEMPLATE_N, TEMPLATES
from .image import Homography, Luma
from .lanes import D_WORD, K_WORD, P_WORD, FrameCells, Lane, decode_lane_counted
from .layout import (
    FINDER_CENTERS,
    GLYPH_BOX,
    RING_COUNT,
    TILE_COUNT,
    TOTAL_SLOTS,
    data_slots,
    gate_slots,
    guard_slots,
    slot_center,
    split_slot,
    tile_center,
)
from .locate import Finder, locate
from .rs import RsError
from .stream import AtomPacket, Beacon, DLane, StreamAssembler, parse_atom_lane, parse_d_lane

__all__ = [
    "DecodeOptions",
    "LaneResult",
    "DecodedFrame",
    "DecodeErrorKind",
    "DecodeError",
    "decode_frame",
    "decode_frame_at",
    "observed_cells",
    "tile_match_error",
    "INK_ON_LIGHT",
    "PINK_ON_DARK",
]

_PATCH = TEMPLATE_N
_CELLS = _PATCH * _PATCH
_HYPOTHESES = 2 * GLYPH_COUNT
#: Relative level of the glyph ink on a light tile (ink / light fill).
INK_ON_LIGHT = 0.04
#: Relative level of a pink glyph on a dark tile (pink / light fill).
PINK_ON_DARK = 0.83
#: Cells cut from each end of a sorted patch to find its robust darkest and brightest level.
_PATCH_CUT = _CELLS // 10
#: Tiles whose contrast is below this fraction of the median tile contrast become erasures
#: in the normalised read.
_WEAK_TILE = 0.25


@dataclass(frozen=True)
class DecodeOptions:
    """Decoder tuning."""

    #: Also try horizontally mirrored images (front-camera previews).
    try_mirrored: bool = True
    #: Blur widths (in template cells) tried for glyph matching.
    template_sigmas: Tuple[float, ...] = (0.0, 0.5, 0.8, 1.1, 1.5)
    #: Largest image (in pixels) the decoder accepts; larger frames should be
    #: downscaled by the caller. Bounds memory and work on hostile input.
    max_pixels: int = 12_000_000

    def __post_init__(self) -> None:
        object.__setattr__(self, "template_sigmas", tuple(float(s) for s in self.template_sigmas))


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

    #: Canvas-to-pixel homography that was used.
    homography: Homography
    #: Orientation: how many quarter turns the code is rotated.
    rotation: int
    #: Whether the image was mirrored.
    mirrored: bool
    #: Lane ``P`` result.
    p: Optional[LaneResult]
    #: Lane ``K`` result.
    k: Optional[LaneResult]
    #: Lane ``D`` result.
    d: Optional[LaneResult]

    @property
    def lanes_ok(self) -> int:
        """Number of lanes that decoded."""
        return (self.p is not None) + (self.k is not None) + (self.d is not None)

    @property
    def lanes(self) -> str:
        """Lanes that decoded, as letters from ``"PKD"``."""
        return "".join(
            letter
            for letter, result in (("P", self.p), ("K", self.k), ("D", self.d))
            if result is not None
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
        for lane, result in ((Lane.P, self.p), (Lane.K, self.k)):
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
        for lane, result in ((Lane.P, self.p), (Lane.K, self.k)):
            if result is not None:
                packet = parse_atom_lane(lane, result.data)
                if packet is not None:
                    assembler.push_atoms(packet)


class DecodeErrorKind(enum.Enum):
    """Why a frame could not be decoded at all."""

    #: The image is empty, smaller than 48 pixels on a side, or larger than
    #: :attr:`DecodeOptions.max_pixels`.
    UNSUPPORTED_IMAGE = "petal image has an unsupported size"
    #: The four corner finders were not found.
    NO_FINDERS = "petal finders not found"
    #: Finders were found but no orientation produced a readable lane.
    NO_ORIENTATION = "no petal orientation produced a readable lane"


class DecodeError(Exception):
    """A frame could not be decoded; :attr:`kind` says why."""

    def __init__(self, kind: DecodeErrorKind) -> None:
        super().__init__(kind.value)
        self.kind = kind


Projector = Callable[[float, float], float]


def _projector(image: Luma, h: Homography) -> Projector:
    """``image.sample(*h.apply(x, y))`` as one fast closure."""
    m0, m1, m2, m3, m4, m5, m6, m7, m8 = h.m
    data = image.data
    w = image.width
    wm1 = w - 1
    hm1 = image.height - 1
    fwm1 = float(wm1)
    fhm1 = float(hm1)

    def project(x: float, y: float) -> float:
        den = m6 * x + m7 * y + m8
        if den == 0.0:
            px = ieee_div(m0 * x + m1 * y + m2, den)
            py = ieee_div(m3 * x + m4 * y + m5, den)
        else:
            px = (m0 * x + m1 * y + m2) / den
            py = (m3 * x + m4 * y + m5) / den
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

    return project


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


def _reference_levels(project: Projector) -> Optional[_Reference]:
    lit = []
    dark = []
    for cx, cy in FINDER_CENTERS:
        total = project(cx, cy)
        for ox, oy in _FINDER_RING:
            total += project(cx + ox, cy + oy)
        lit_level = total / 9.0
        sx = 1.0 if cx < 512.0 else -1.0
        sy = 1.0 if cy < 512.0 else -1.0
        a = _dot_samples(project, cx + sx * 100.0, cy, 5.0)
        b = _dot_samples(project, cx, cy + sy * 100.0, 5.0)
        dark_level = 0.5 * (a + b)
        if lit_level - dark_level < 12.0:
            return None
        lit.append(lit_level)
        dark.append(dark_level)
    return _Reference(tuple(lit), tuple(dark))


_SLOT_POINTS = tuple(slot_center(*split_slot(flat)) for flat in range(TOTAL_SLOTS))
_GATES = gate_slots()
_GUARDS = guard_slots()
_GATES_BY_RING = tuple(tuple(s for s in _GATES if split_slot(s)[0] == ring) for ring in range(3))
_GUARDS_BY_RING = tuple(tuple(s for s in _GUARDS if split_slot(s)[0] == ring) for ring in range(3))
_DATA_SLOTS = data_slots()
_DATA_RINGS = tuple(split_slot(slot)[0] for slot in _DATA_SLOTS)


def _normalised_dot(project: Projector, reference: _Reference, flat: int) -> float:
    x, y = _SLOT_POINTS[flat]
    lit, dark = reference.at(x, y)
    return ieee_div(_dot_samples(project, x, y, 3.5) - dark, lit - dark)


def _sequential_sum(values: Sequence[float]) -> float:
    """Left-to-right float sum, as Rust's ``Iterator::sum`` computes it."""
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


def _decode_with_erasures(
    lane: Lane, word: bytes, confidence: Sequence[float]
) -> Optional[LaneResult]:
    """Try Reed-Solomon with growing numbers of erasures, least confident first.

    The schedule erases 0, 1/8, 1/4, 1/3 and 1/2 of the parity bytes, and for lane ``K`` also
    2/3. Lanes ``D`` and ``P`` stop at 1/2: their words have only 11 and 13 parity bytes, and a
    further erasure step leaves so few spare ones that it accepts wrong codewords (lane ``D``
    at 7 erasures: about 0.4 % of random words, and 5 wrong lanes in 2 900 simulated harsh
    frames; lane ``P`` at 8: 2 wrong lanes in 600 banded 480p frames). Capping them costs
    0.65 % of the lane ``D`` reads and 0.15 % of the lane ``P`` reads in those frames.
    """
    nsym = lane.parity_len
    order = sorted(range(len(word)), key=lambda i: total_key(confidence[i]))
    counts = [0, nsym // 8, nsym // 4, nsym // 3, nsym // 2]
    if lane is Lane.K:
        counts.append(nsym * 2 // 3)
    schedule = []
    for count in counts:
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


def _read_lane_d(project: Projector, reference: _Reference) -> Optional[LaneResult]:
    """Lane ``D`` under one pose."""
    word, confidence = _read_dots(project, reference)
    return _decode_with_erasures(Lane.D, word, confidence)


_PATTERNS: Dict[float, Tuple[Tuple[float, ...], ...]] = {}


def _build_patterns(sigma: float) -> Tuple[Tuple[float, ...], ...]:
    """``pred[b * 16 + g]``: expected normalised patch for polarity ``b``, glyph ``g``."""
    cached = _PATTERNS.get(sigma)
    if cached is not None:
        return cached
    raw = []
    for i in range(-2, 3):
        if sigma < 0.05:
            raw.append(1.0 if i == 0 else 0.0)
        else:
            raw.append(math.exp(-(float(i) * float(i)) / (2.0 * sigma * sigma)))
    total = _sequential_sum(raw)
    kernel = [v / total for v in raw]
    pred = []
    for polarity in range(2):
        for template in TEMPLATES:
            coverage = [c / 255.0 for c in template]
            blurred = [0.0] * _CELLS
            for v in range(_PATCH):
                for u in range(_PATCH):
                    acc = 0.0
                    for ky, wy in enumerate(kernel):
                        sy = v + ky - 2
                        if not 0 <= sy < _PATCH:
                            continue
                        for kx, wx in enumerate(kernel):
                            sx = u + kx - 2
                            if 0 <= sx < _PATCH:
                                acc += wx * wy * coverage[sy * _PATCH + sx]
                    blurred[v * _PATCH + u] = acc
            if polarity == 1:
                pattern = tuple(1.0 - (1.0 - INK_ON_LIGHT) * ink for ink in blurred)
            else:
                pattern = tuple(PINK_ON_DARK * ink for ink in blurred)
            pred.append(pattern)
    patterns = tuple(pred)
    if sigma == sigma:
        _PATTERNS[sigma] = patterns
    return patterns


class _TileRead:
    __slots__ = ("light", "glyph", "polarity_margin", "glyph_margin", "error")

    def __init__(
        self, light: bool, glyph: int, polarity_margin: float, glyph_margin: float, error: float
    ) -> None:
        self.light = light
        self.glyph = glyph
        self.polarity_margin = polarity_margin
        self.glyph_margin = glyph_margin
        self.error = error


_HALF_BOX = GLYPH_BOX / 2.0
_CELL = GLYPH_BOX / _PATCH
_QUARTER = ((-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25))
_TILE_CENTERS = tuple(tile_center(t) for t in range(TILE_COUNT))
_NOT_ERASED = (False,) * TILE_COUNT


def _sample_patches(project: Projector) -> List[List[float]]:
    """The raw 8x8 luma patch of every tile, as captured (no reference levels applied).

    ``patches[tile][v * 8 + u]`` is the mean of four bilinear samples at +-1/4 cell.
    """
    cell = _CELL
    half = _HALF_BOX
    offsets = tuple((ox * cell, oy * cell) for ox, oy in _QUARTER)
    (ax, ay), (bx, by), (cx_, cy_), (dx, dy) = offsets
    patches: List[List[float]] = []
    for cx, cy in _TILE_CENTERS:
        patch: List[float] = []
        for v in range(_PATCH):
            gy = cy - half + (v + 0.5) * cell
            for u in range(_PATCH):
                gx = cx - half + (u + 0.5) * cell
                total = project(gx + ax, gy + ay)
                total += project(gx + bx, gy + by)
                total += project(gx + cx_, gy + cy_)
                total += project(gx + dx, gy + dy)
                patch.append(total / 4.0)
        patches.append(patch)
    return patches


def _f64_min(a: float, b: float) -> float:
    """Rust's ``f64::min``: a NaN operand yields the other operand."""
    if a != a:
        return b
    if b != b:
        return a
    return a if a < b else b


def _sorted_total(values: Sequence[float]) -> List[float]:
    """``values`` ascending in Rust's ``f64::total_cmp`` order."""
    total = sum(values)
    if total == total:
        # no NaN: the plain order is the total order (zeros of either sign tie, which no
        # result below can tell apart)
        return sorted(values)
    return sorted(values, key=total_key)


def _patch_levels(values: Sequence[float]) -> Tuple[float, float]:
    """The robust darkest and brightest level of a patch: the values ``_PATCH_CUT`` cells in
    from either end of the sorted cells."""
    ordered = _sorted_total(values)
    return ordered[_PATCH_CUT], ordered[_CELLS - 1 - _PATCH_CUT]


def _apply_levels(values: Sequence[float], low: float, high: float, floor: float) -> List[float]:
    """``values`` with ``low`` mapped to 0 and ``high`` to 1, clamped to [-0.25, 1.25]."""
    span = high - low
    if not span > floor:  # `f64::max`: a NaN span yields the floor
        span = floor
    out = []
    for value in values:
        level = (value - low) / span
        if level < -0.25:  # `f64::clamp`: NaN passes through
            level = -0.25
        elif level > 1.25:
            level = 1.25
        out.append(level)
    return out


def _rescale(values: Sequence[float], floor: float) -> List[float]:
    """Maps a patch's own darkest level to 0 and brightest to 1, with ``floor`` as the smallest
    span that counts as contrast."""
    low, high = _patch_levels(values)
    return _apply_levels(values, low, high, floor)


_RESCALED_PATTERNS: Dict[float, Tuple[Tuple[float, ...], ...]] = {}


def _rescaled_patterns(sigma: float) -> Tuple[Tuple[float, ...], ...]:
    """The templates of ``sigma`` rescaled like the patches of the normalised read."""
    cached = _RESCALED_PATTERNS.get(sigma)
    if cached is not None:
        return cached
    patterns = tuple(tuple(_rescale(pattern, 0.001)) for pattern in _build_patterns(sigma))
    if sigma == sigma:
        _RESCALED_PATTERNS[sigma] = patterns
    return patterns


def _classify(
    patches: Sequence[Sequence[float]],
    sigmas: Sequence[float],
    rescale_templates: bool,
    erased: Sequence[bool],
) -> List[_TileRead]:
    """Picks for every patch the polarity and glyph whose template matches best.

    The template blur is chosen per frame, by the lowest total error. With
    ``rescale_templates`` the templates are rescaled like the patches; tiles flagged in
    ``erased`` get zero margins, so they are the first the Reed-Solomon decoder treats as
    erasures.
    """
    best_total = F64_MAX
    best_reads: List[_TileRead] = []
    tiles = len(patches)
    if tiles == 0:
        return best_reads
    # cell-major, so that one cell of every tile is compared at a time
    cells = list(zip(*patches))
    for sigma in sigmas:
        pred = _rescaled_patterns(sigma) if rescale_templates else _build_patterns(sigma)
        # errors[h * tiles + t] accumulates sum_c (patch[t][c] - pred[h][c])^2
        # over c in order, exactly like the reference's per-tile fold.
        errors: Optional[List[float]] = None
        for c in range(_CELLS):
            observed = cells[c] * _HYPOTHESES
            expected: List[float] = []
            for h in range(_HYPOTHESES):
                expected += [pred[h][c]] * tiles
            diff = list(map(sub, observed, expected))
            squares = map(mul, diff, diff)
            errors = list(squares) if errors is None else list(map(add, errors, squares))
        assert errors is not None
        total = 0.0
        reads = []
        hypotheses = range(_HYPOTHESES)
        for tile in range(tiles):
            row = errors[tile::tiles]
            best = min(hypotheses, key=row.__getitem__)
            polarity = best // GLYPH_COUNT
            glyph = best % GLYPH_COUNT
            error = row[best]
            other = F64_MAX
            start = (1 - polarity) * GLYPH_COUNT
            for value in row[start : start + GLYPH_COUNT]:
                if value < other:
                    other = value
            other_glyph = F64_MAX
            start = polarity * GLYPH_COUNT
            for g in range(GLYPH_COUNT):
                if g != glyph:
                    value = row[start + g]
                    if value < other_glyph:
                        other_glyph = value
            total += error
            if erased[tile]:
                reads.append(_TileRead(polarity == 1, glyph, 0.0, 0.0, error))
            else:
                reads.append(
                    _TileRead(polarity == 1, glyph, other - error, other_glyph - error, error)
                )
        if total < best_total:
            best_total = total
            best_reads = reads
    return best_reads


def _read_tiles(
    patches: Sequence[Sequence[float]], reference: _Reference, sigmas: Sequence[float]
) -> List[_TileRead]:
    """The level read: every patch is judged against the light and dark levels measured at
    the finders, interpolated to the tile."""
    levelled = []
    for raw, (cx, cy) in zip(patches, _TILE_CENTERS):
        lit, dark = reference.at(cx, cy)
        scale = lit - dark
        if scale != 0.0:
            levelled.append([(value - dark) / scale for value in raw])
        else:
            levelled.append([ieee_div(value - dark, scale) for value in raw])
    return _classify(levelled, sigmas, False, _NOT_ERASED)


def _read_tiles_normalised(
    patches: Sequence[Sequence[float]], sigmas: Sequence[float]
) -> List[_TileRead]:
    """The normalised read: every patch and every template is rescaled by its own contrast
    before they are compared, so the judgement does not depend on absolute levels."""
    levels = [_patch_levels(raw) for raw in patches]
    spans = [high - low for low, high in levels]
    median = _sorted_total(spans)[TILE_COUNT // 2]
    weak = _WEAK_TILE * median
    erased = [span < weak for span in spans]
    # `_rescale(raw, 1.0)` with the levels found above (the same sort, done once)
    scaled = [_apply_levels(raw, low, high, 1.0) for raw, (low, high) in zip(patches, levels)]
    return _classify(scaled, sigmas, True, erased)


WordConfidence = Tuple[bytearray, List[float]]


def _tile_words(reads: Sequence[_TileRead]) -> Tuple[WordConfidence, WordConfidence]:
    p = bytearray(P_WORD)
    p_conf = [F64_MAX] * P_WORD
    k = bytearray(K_WORD)
    k_conf = [F64_MAX] * K_WORD
    for tile, read in enumerate(reads):
        if read.light:
            p[tile >> 3] |= 1 << (7 - (tile & 7))
        p_conf[tile >> 3] = _f64_min(p_conf[tile >> 3], read.polarity_margin)
        k[tile >> 1] |= read.glyph << 4 if tile % 2 == 0 else read.glyph
        k_conf[tile >> 1] = _f64_min(
            k_conf[tile >> 1], _f64_min(read.glyph_margin, read.polarity_margin)
        )
    return (p, p_conf), (k, k_conf)


def _read_tile_lanes(
    patches: Sequence[Sequence[float]], reference: _Reference, sigmas: Sequence[float]
) -> Tuple[Optional[LaneResult], Optional[LaneResult]]:
    """Lanes ``P`` and ``K`` from one set of patches: the level read first, then, for any
    lane still unreadable, the normalised read."""
    reads = _read_tiles(patches, reference, sigmas)
    (p_word, p_conf), (k_word, k_conf) = _tile_words(reads)
    p = _decode_with_erasures(Lane.P, p_word, p_conf)
    k = _decode_with_erasures(Lane.K, k_word, k_conf)
    if p is None or k is None:
        reads = _read_tiles_normalised(patches, sigmas)
        (p_word, p_conf), (k_word, k_conf) = _tile_words(reads)
        if p is None:
            p = _decode_with_erasures(Lane.P, p_word, p_conf)
        if k is None:
            k = _decode_with_erasures(Lane.K, k_word, k_conf)
    return p, k


_CANONICAL = tuple((float(x), float(y)) for x, y in FINDER_CENTERS)


def _hypotheses(
    finders: Sequence[Finder], try_mirrored: bool
) -> List[Tuple[int, bool, Homography]]:
    out = []
    for mirrored in (False, True):
        if mirrored and not try_mirrored:
            continue
        for rotation in range(4):
            dst = []
            for i in range(4):
                q = finders[(rotation + 4 - i) % 4] if mirrored else finders[(i + rotation) % 4]
                dst.append((q.x, q.y))
            h = Homography.from_points(_CANONICAL, dst)
            if h is not None:
                out.append((rotation, mirrored, h))
    return out


def _supported(image: Luma, options: DecodeOptions) -> bool:
    return (
        image.width >= 48
        and image.height >= 48
        and image.width * image.height <= options.max_pixels
        and len(image.data) == image.width * image.height
    )


def _finish(
    project: Projector,
    options: DecodeOptions,
    rotation: int,
    mirrored: bool,
    h: Homography,
    reference: _Reference,
    d: Optional[LaneResult],
) -> DecodedFrame:
    if d is None:
        d = _read_lane_d(project, reference)
    patches = _sample_patches(project)
    p, k = _read_tile_lanes(patches, reference, options.template_sigmas)
    return DecodedFrame(homography=h, rotation=rotation, mirrored=mirrored, p=p, k=k, d=d)


def _decode(image: Luma, options: DecodeOptions) -> DecodedFrame:
    if not _supported(image, options):
        raise DecodeError(DecodeErrorKind.UNSUPPORTED_IMAGE)
    finders = locate(image)
    if finders is None:
        raise DecodeError(DecodeErrorKind.NO_FINDERS)
    scored = []
    for rotation, mirrored, h in _hypotheses(finders, options.try_mirrored):
        project = _projector(image, h)
        reference = _reference_levels(project)
        if reference is None:
            continue
        score = _gate_score(project, reference)
        scored.append((score, rotation, mirrored, h, reference, project))
    # stable sort, highest score first (Rust: `b.0.total_cmp(&a.0)`)
    scored.sort(key=lambda entry: total_key(entry[0]), reverse=True)
    # 1. the ring beacon is the cheapest and strongest orientation check
    for score, rotation, mirrored, h, reference, project in scored[:3]:
        if score < 0.2:
            break
        d = _read_lane_d(project, reference)
        if d is not None:
            return _finish(project, options, rotation, mirrored, h, reference, d)
    # 2. fall back to the tile lanes under the most promising orientations
    for _, rotation, mirrored, h, reference, project in scored[:4]:
        patches = _sample_patches(project)
        p, k = _read_tile_lanes(patches, reference, options.template_sigmas)
        if p is not None or k is not None:
            d = _read_lane_d(project, reference)
            return DecodedFrame(homography=h, rotation=rotation, mirrored=mirrored, p=p, k=k, d=d)
    raise DecodeError(DecodeErrorKind.NO_ORIENTATION)


def _scale(h: Homography, factor: int) -> Homography:
    f = float(factor)
    return Homography((f, 0.0, 0.0, 0.0, f, 0.0, 0.0, 0.0, 1.0)).compose(h)


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

    Raises :class:`DecodeError` with :attr:`DecodeErrorKind.UNSUPPORTED_IMAGE`
    for unusable images, :attr:`DecodeErrorKind.NO_FINDERS` when no code is
    visible and :attr:`DecodeErrorKind.NO_ORIENTATION` when no orientation
    yields a readable lane.

    ``max_side`` (off by default) first box-filters images whose longer side
    exceeds it by the smallest integer factor that fits; the returned
    homography still maps into the original image's pixel coordinates.
    """
    if options is None:
        options = DecodeOptions()
    factor = _downscale_factor(image, max_side)
    if factor == 1:
        return _decode(image, options)
    frame = _decode(image.downscaled(factor), options)
    return DecodedFrame(
        homography=_scale(frame.homography, factor),
        rotation=frame.rotation,
        mirrored=frame.mirrored,
        p=frame.p,
        k=frame.k,
        d=frame.d,
    )


def decode_frame_at(
    image: Luma, homography: Homography, options: Optional[DecodeOptions] = None
) -> Optional[DecodedFrame]:
    """Read all lanes with a known canvas-to-pixel homography (no finder search).

    Returns ``None`` when the image is unusable (see
    :attr:`DecodeErrorKind.UNSUPPORTED_IMAGE`) or the finder reference levels
    are too weak. Used by trackers that already know the pose, by refinement
    passes and by qualification tooling with a ground-truth pose.
    """
    if options is None:
        options = DecodeOptions()
    if not _supported(image, options):
        return None
    project = _projector(image, homography)
    reference = _reference_levels(project)
    if reference is None:
        return None
    return _finish(project, options, 0, False, homography, reference, None)


def observed_cells(
    image: Luma, frame: DecodedFrame, options: Optional[DecodeOptions] = None
) -> Optional[FrameCells]:
    """Build the cells a decoder believes it saw, for diagnostics."""
    if options is None:
        options = DecodeOptions()
    if not _supported(image, options):
        return None
    project = _projector(image, frame.homography)
    reference = _reference_levels(project)
    if reference is None:
        return None
    patches = _sample_patches(project)
    reads = _read_tiles(patches, reference, options.template_sigmas)
    (p, _), (k, _) = _tile_words(reads)
    d, _ = _read_dots(project, reference)
    return FrameCells.from_words(bytes(p), bytes(k), bytes(d))


def tile_match_error(
    image: Luma, frame: DecodedFrame, options: Optional[DecodeOptions] = None
) -> Optional[float]:
    """Mean squared tile-match error, a quick image-quality indicator."""
    if options is None:
        options = DecodeOptions()
    if not _supported(image, options):
        return None
    project = _projector(image, frame.homography)
    reference = _reference_levels(project)
    if reference is None:
        return None
    patches = _sample_patches(project)
    reads = _read_tiles(patches, reference, options.template_sigmas)
    if not reads:
        return NAN
    return _sequential_sum([r.error for r in reads]) / len(reads)
