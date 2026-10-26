# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The sixteen katakana of the band tiles.

A band tile's glyph is a four-bit symbol. The alphabet is the subset of the
Iroha ordering ``イロハニホヘト...ス`` whose members stay most distinguishable
after camera blur: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol ``0`` is
``イ`` and symbol ``15`` is ``ン``.

Glyphs are defined as stroke polylines on a 32 x 32 grid (``y`` grows downward)
drawn with round caps and joins of width :data:`STROKE_WIDTH`. A point is inked
when it lies within half a stroke width of a stroke segment (:func:`is_inked`);
renderers and the band reader's templates both follow that definition, so
classification never depends on a rasteriser.

:func:`inked_flags` classifies a whole row of points at once, exactly as
:func:`is_inked` would one by one: it is what the renderer and the reader's
template rasters use.
"""

from __future__ import annotations

import math
from bisect import bisect_left, bisect_right
from typing import List, Optional, Sequence, Tuple

__all__ = [
    "GLYPH_COUNT",
    "GLYPH_GRID",
    "STROKE_WIDTH",
    "GLYPH_CHARS",
    "STROKES",
    "is_inked",
    "inked_flags",
]

#: Number of glyphs (symbols) in the alphabet.
GLYPH_COUNT = 16
#: Side of the glyph design grid.
GLYPH_GRID = 32.0
#: Stroke width on the design grid.
STROKE_WIDTH = 6.5

#: The sixteen characters in symbol order.
GLYPH_CHARS = "イロハニヘトワカレムノケアヒスン"

#: Stroke definitions for every glyph, in symbol order: each glyph is a tuple of strokes and
#: each stroke a polyline of ``(x, y)`` points on the design grid.
STROKES: Tuple[Tuple[Tuple[Tuple[float, float], ...], ...], ...] = (
    # イ
    (((22.0, 4.0), (8.0, 18.0)), ((19.0, 10.0), (19.0, 29.0))),
    # ロ
    (((6.0, 6.0), (26.0, 6.0), (26.0, 26.0), (6.0, 26.0), (6.0, 6.0)),),
    # ハ
    (((14.0, 6.0), (6.0, 27.0)), ((18.0, 10.0), (27.0, 27.0))),
    # ニ
    (((8.0, 10.0), (24.0, 10.0)), ((4.0, 24.0), (28.0, 24.0))),
    # ヘ
    (((3.0, 19.0), (12.0, 10.0), (29.0, 24.0)),),
    # ト
    (((11.0, 3.0), (11.0, 29.0)), ((11.0, 13.0), (26.0, 21.0))),
    # ワ
    (((7.0, 21.0), (7.0, 8.0), (25.0, 8.0), (23.0, 19.0), (10.0, 29.0)),),
    # カ
    (
        ((14.0, 3.0), (14.0, 16.0), (8.0, 28.0)),
        ((4.0, 12.0), (25.0, 12.0), (25.0, 23.0), (21.0, 28.0)),
    ),
    # レ
    (((9.0, 4.0), (9.0, 27.0), (27.0, 8.0)),),
    # ム
    (((17.0, 4.0), (7.0, 23.0), (27.0, 24.0)), ((21.0, 16.0), (26.0, 22.0))),
    # ノ
    (((22.0, 4.0), (17.0, 16.0), (9.0, 28.0)),),
    # ケ
    (
        ((13.0, 3.0), (6.0, 13.0)),
        ((10.0, 12.0), (27.0, 12.0)),
        ((19.0, 12.0), (16.0, 22.0), (9.0, 29.0)),
    ),
    # ア
    (
        ((5.0, 10.0), (26.0, 10.0), (25.0, 18.0), (19.0, 24.0)),
        ((15.0, 10.0), (15.0, 21.0), (8.0, 29.0)),
    ),
    # ヒ
    (((10.0, 5.0), (10.0, 26.0), (26.0, 26.0)), ((10.0, 14.0), (25.0, 14.0))),
    # ス
    (((6.0, 5.0), (24.0, 5.0), (17.0, 15.0), (6.0, 27.0)), ((13.0, 15.0), (28.0, 28.0))),
    # ン
    (((6.0, 7.0), (12.0, 13.0)), ((6.0, 25.0), (14.0, 27.0), (27.0, 9.0))),
)

assert len(STROKES) == GLYPH_COUNT and len(GLYPH_CHARS) == GLYPH_COUNT

#: Half the stroke width: a point within this distance of a segment is inked.
_RADIUS = STROKE_WIDTH / 2.0

#: Every glyph's segments as ``(ax, ay, bx, by, dx, dy, length_sq)``, in stroke order.
_SEGMENTS = tuple(
    tuple(
        (ax, ay, bx, by, bx - ax, by - ay, (bx - ax) * (bx - ax) + (by - ay) * (by - ay))
        for stroke in strokes
        for (ax, ay), (bx, by) in zip(stroke, stroke[1:])
    )
    for strokes in STROKES
)


def _check_glyph(glyph: int) -> int:
    if not 0 <= glyph < GLYPH_COUNT:
        raise IndexError("glyph index out of range")
    return glyph


def _inked(segments: Sequence[Tuple[float, ...]], px: float, py: float) -> bool:
    """The reference's test, segment by segment: the distance from ``(px, py)`` to a
    segment is ``|p - (a + t (b - a))|`` with ``t`` the clamped projection."""
    radius = _RADIUS
    for ax, ay, _bx, _by, dx, dy, length_sq in segments:
        if length_sq == 0.0:
            t = 0.0
        else:
            t = ((px - ax) * dx + (py - ay) * dy) / length_sq
            if t < 0.0:
                t = 0.0
            elif t > 1.0:
                t = 1.0
        ex = px - (ax + t * dx)
        ey = py - (ay + t * dy)
        if math.sqrt(ex * ex + ey * ey) <= radius:
            return True
    return False


def is_inked(glyph: int, x: float, y: float) -> bool:
    """Return whether design-grid point ``(x, y)`` is inked in ``glyph``: whether it lies
    within half a stroke width of one of its stroke segments."""
    return _inked(_SEGMENTS[_check_glyph(glyph)], x, y)


#: The tolerance, in design units, by which :func:`inked_flags` widens and narrows the stroke
#: radius and its spans before it trusts them; far above the rounding of either.
_SLACK = 1e-6


def _capsule_span(
    ax: float, ay: float, bx: float, by: float, y: float, radius: float
) -> Optional[Tuple[float, float]]:
    """Approximately the ``x`` interval of the horizontal line at ``y`` that lies within
    ``radius`` of the segment ``a``-``b``: the union of the discs around its ends and the
    strip along it (a convex set meets a line in one interval), or ``None``."""
    lo = math.inf
    hi = -math.inf
    r2 = radius * radius
    e = y - ay
    f = y - by
    q = r2 - e * e
    if q >= 0.0:
        h = math.sqrt(q)
        lo = min(lo, ax - h)
        hi = max(hi, ax + h)
    q = r2 - f * f
    if q >= 0.0:
        h = math.sqrt(q)
        lo = min(lo, bx - h)
        hi = max(hi, bx + h)
    dx = bx - ax
    dy = by - ay
    if dy == 0.0:
        if abs(e) <= radius:
            lo = min(lo, ax, bx)
            hi = max(hi, ax, bx)
    else:
        length_sq = dx * dx + dy * dy
        # within the strip: |(x - ax) dy - e dx| <= radius |b - a|
        centre = ax + e * dx / dy
        half = radius * math.sqrt(length_sq) / abs(dy)
        start = centre - half
        end = centre + half
        # and projecting inside the segment: 0 <= ((x - ax) dx + e dy) / |b - a|^2 <= 1
        if dx != 0.0:
            x0 = ax - e * dy / dx
            x1 = ax + (length_sq - e * dy) / dx
            start = max(start, min(x0, x1))
            end = min(end, max(x0, x1))
        elif not 0.0 <= e * dy / length_sq <= 1.0:
            start, end = math.inf, -math.inf
        if start <= end:
            lo = min(lo, start)
            hi = max(hi, end)
    return (lo, hi) if lo <= hi else None


def inked_flags(glyph: int, y: float, xs: Sequence[float]) -> bytearray:
    """One flag per point ``(xs[i], y)`` of the design grid (``xs`` ascending): ``1`` when
    :func:`is_inked` holds for it, ``0`` when not.

    Each segment's span along the row is computed twice, for a stroke radius a hair
    narrower and a hair wider than the real one: points well inside a narrow span are
    inked, points outside every wide span are not, and the few between are decided by
    :func:`is_inked` itself, so the flags are exactly the reference's.
    """
    segments = _SEGMENTS[_check_glyph(glyph)]
    count = len(xs)
    flags = bytearray(count)
    borderline: List[Tuple[int, int]] = []
    for ax, ay, bx, by, _dx, _dy, _length_sq in segments:
        outer = _capsule_span(ax, ay, bx, by, y, _RADIUS + _SLACK)
        if outer is None:
            continue
        first = bisect_left(xs, outer[0] - _SLACK)
        last = bisect_right(xs, outer[1] + _SLACK)
        if first >= last:
            continue
        inner = _capsule_span(ax, ay, bx, by, y, _RADIUS - _SLACK)
        if inner is None:
            borderline.append((first, last))
            continue
        start = max(bisect_left(xs, inner[0] + _SLACK), first)
        end = min(bisect_right(xs, inner[1] - _SLACK), last)
        if start >= end:
            borderline.append((first, last))
            continue
        flags[start:end] = b"\x01" * (end - start)
        if first < start:
            borderline.append((first, start))
        if end < last:
            borderline.append((end, last))
    for first, last in borderline:
        for i in range(first, last):
            if not flags[i] and _inked(segments, xs[i], y):
                flags[i] = 1
    return flags
