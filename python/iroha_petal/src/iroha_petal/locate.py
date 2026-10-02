# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Finding the four corner finders in a camera luma plane.

Pipeline: adaptive threshold (local mean via box sums) -> 4-connected component
labelling -> blossom detection (a large, round, isolated blob) -> selection of
the four finders that form a plausible, similarly sized quadrilateral. Solid
blossoms survive defocus that would fill in the gaps of a ring-shaped marker.

The implementation is a function-by-function port of the Rust reference with
pure-Python performance in mind: box sums slide over rows with C-level
``map``/``itemgetter`` passes, and components are labelled run by run. Both
produce exactly the reference's results (same thresholds, same components in
the same order, same floating-point moments).
"""

from __future__ import annotations

import math
import re
from array import array
from collections import Counter
from dataclasses import dataclass
from itertools import accumulate, repeat
from operator import add, and_, gt, itemgetter, mul, sub, truediv
from typing import List, Optional, Sequence, Tuple

from ._numeric import F64_MAX, ieee_div, total_key
from .image import Luma

__all__ = [
    "Finder",
    "Component",
    "adaptive_binarize",
    "label_components",
    "blossoms",
    "select_quad",
    "refine_center",
    "locate",
    "SENSITIVITIES",
]

#: Threshold sensitivities tried by :func:`locate`, mildest first.
SENSITIVITIES = (0.12, 0.22, 0.34)


@dataclass(frozen=True)
class Finder:
    """A detected finder."""

    #: Centre ``x`` in pixel-edge coordinates.
    x: float
    #: Centre ``y`` in pixel-edge coordinates.
    y: float
    #: Apparent outer diameter in pixels.
    size: float


class Component:
    """A connected component of the binarised image."""

    __slots__ = (
        "area",
        "min_x",
        "max_x",
        "min_y",
        "max_y",
        "sum_x",
        "sum_y",
        "sum_xx",
        "sum_yy",
        "sum_xy",
    )

    def __init__(
        self,
        area: int,
        min_x: int,
        max_x: int,
        min_y: int,
        max_y: int,
        sum_x: float,
        sum_y: float,
        sum_xx: float,
        sum_yy: float,
        sum_xy: float,
    ) -> None:
        #: Pixel count.
        self.area = area
        #: Left-most pixel column.
        self.min_x = min_x
        #: Right-most pixel column.
        self.max_x = max_x
        #: Top-most pixel row.
        self.min_y = min_y
        #: Bottom-most pixel row.
        self.max_y = max_y
        # Raw moments over pixel centres (x + 0.5, y + 0.5).
        self.sum_x = sum_x
        self.sum_y = sum_y
        self.sum_xx = sum_xx
        self.sum_yy = sum_yy
        self.sum_xy = sum_xy

    def width(self) -> float:
        """Bounding-box width in pixels."""
        return float(self.max_x - self.min_x + 1)

    def height(self) -> float:
        """Bounding-box height in pixels."""
        return float(self.max_y - self.min_y + 1)

    def centroid(self) -> Tuple[float, float]:
        """Centre of mass in pixel-edge coordinates."""
        return self.sum_x / self.area, self.sum_y / self.area

    def axis_ratio(self) -> float:
        """Ratio of the smaller to the larger principal axis of the blob."""
        n = float(self.area)
        cx, cy = self.centroid()
        vxx = self.sum_xx / n - cx * cx
        vyy = self.sum_yy / n - cy * cy
        vxy = self.sum_xy / n - cx * cy
        mean = 0.5 * (vxx + vyy)
        d = vxx - vyy
        spread = math.sqrt(0.25 * (d * d) + vxy * vxy)
        major = mean + spread
        minor = mean - spread
        if not minor > 0.0:
            minor = 0.0
        if major <= 0.0:
            return 0.0
        return math.sqrt(minor / major)

    def __repr__(self) -> str:
        return (
            f"Component(area={self.area}, x={self.min_x}..{self.max_x}, "
            f"y={self.min_y}..{self.max_y})"
        )


class _Threshold:
    """Everything the adaptive threshold needs that does not depend on sensitivity."""

    __slots__ = ("width", "height", "rows", "means", "low", "range", "floor")

    def __init__(self, image: Luma) -> None:
        w = image.width
        h = image.height
        data = bytes(image.data)
        self.width = w
        self.height = h
        self.rows = [data[y * w : (y + 1) * w] for y in range(h)]
        counts = Counter(data)
        histogram = [counts.get(level, 0) for level in range(256)]
        total = float(len(data))

        def percentile(p: float) -> float:
            target = total * p
            seen = 0.0
            for level, count in enumerate(histogram):
                seen += count
                if seen >= target:
                    return float(level)
            return 255.0

        low = percentile(0.02)
        high = percentile(0.995)
        value_range = high - low
        if not value_range > 8.0:
            value_range = 8.0
        self.low = low
        self.range = value_range
        self.floor = low + 0.2 * value_range
        radius = min(max(min(w, h) // 8, 12), 64)
        self.means = self._local_means(radius)

    def _local_means(self, radius: int) -> List[array]:
        """Mean of the ``(2r+1)^2`` window (clipped to the image) around every pixel."""
        w = self.width
        h = self.height
        rows = self.rows
        if w == 0:
            return [array("d") for _ in range(h)]
        x0s = [max(x - radius, 0) for x in range(w)]
        x1s = [min(x + radius + 1, w) for x in range(w)]
        widths = [b - a for a, b in zip(x0s, x1s)]
        if w >= 2:
            take1 = itemgetter(*x1s)
            take0 = itemgetter(*x0s)
        else:

            def take1(values, _i=x1s[0]):
                return (values[_i],)

            def take0(values, _i=x0s[0]):
                return (values[_i],)

        columns = [0] * w
        for y in range(min(radius + 1, h)):
            columns = list(map(add, columns, rows[y]))
        means = []
        for y in range(h):
            y0 = max(y - radius, 0)
            y1 = min(y + radius + 1, h)
            prefix = list(accumulate(columns, initial=0))
            sums = map(sub, take1(prefix), take0(prefix))
            areas = map(mul, widths, repeat(y1 - y0))
            # exact integers, correctly rounded division: identical to the
            # reference's `sum as f64 / area as f64`
            means.append(array("d", map(truediv, sums, areas)))
            if y + radius + 1 < h:
                columns = list(map(add, columns, rows[y + radius + 1]))
            if y - radius >= 0:
                columns = list(map(sub, columns, rows[y - radius]))
        return means

    def mask_rows(self, sensitivity: float) -> List[bytes]:
        """One ``0``/``1`` byte row per image row for ``sensitivity``."""
        margin = sensitivity * self.range
        if not margin > 5.0:
            margin = 5.0
        floor = self.floor
        bright = bytes(1 if level > floor else 0 for level in range(256))
        out = []
        means = self.means
        for y, row in enumerate(self.rows):
            candidates = row.translate(bright)
            if 1 not in candidates:
                out.append(candidates)
                continue
            above = bytes(map(gt, row, map(add, means[y], repeat(margin))))
            out.append(bytes(map(and_, above, candidates)))
        return out


def adaptive_binarize(image: Luma, sensitivity: float) -> bytearray:
    """Mark pixels that are clearly brighter than their neighbourhood.

    ``sensitivity`` scales the margin above the local mean in units of the
    image's dynamic range (about 0.12 for faint codes, larger values separate
    blurred finder rings from their cores). Returns one ``0``/``1`` byte per
    pixel, row-major.
    """
    return bytearray(b"".join(_Threshold(image).mask_rows(sensitivity)))


_RUN = re.compile(b"\x01+")


def _label_rows(rows: Sequence[bytes], width: int) -> List[Component]:
    """Label 4-connected components of ``0``/``1`` rows, run by run.

    A component's root is the smallest provisional label in it, which is the
    label of its first run in raster order, so components come out ordered by
    their first pixel exactly like the reference's pixel-by-pixel union-find.
    Moments are accumulated as exact integers (scaled by 2 or 4) and converted
    once, which equals the reference's exact floating-point sums.
    """
    parent = [0]
    area = [0]
    min_x = [0]
    max_x = [0]
    min_y = [0]
    max_y = [0]
    sx2 = [0]
    sy2 = [0]
    sxx4 = [0]
    syy4 = [0]
    sxy4 = [0]
    # odd_squares[m] = sum of (2x + 1)^2 for x < m
    odd_squares = [m * (2 * m - 1) * (2 * m + 1) // 3 for m in range(width + 1)]

    def find(label: int) -> int:
        while parent[label] != label:
            parent[label] = parent[parent[label]]
            label = parent[label]
        return label

    previous: List[Tuple[int, int, int]] = []
    finditer = _RUN.finditer
    for y, row in enumerate(rows):
        if 1 not in row:
            previous = []
            continue
        current = []
        j = 0
        count = len(previous)
        oy = 2 * y + 1
        for match in finditer(row):
            s, e = match.span()
            while j < count and previous[j][1] <= s:
                j += 1
            label = 0
            k = j
            while k < count:
                ps, pe, pl = previous[k]
                if ps >= e:
                    break
                root = find(pl)
                if label == 0:
                    label = root
                elif root < label:
                    parent[label] = root
                    label = root
                elif root > label:
                    parent[root] = label
                k += 1
            n = e - s
            if label == 0:
                label = len(parent)
                parent.append(label)
                area.append(n)
                min_x.append(s)
                max_x.append(e - 1)
                min_y.append(y)
                max_y.append(y)
                sx2.append((s + e) * n)
                sy2.append(n * oy)
                sxx4.append(odd_squares[e] - odd_squares[s])
                syy4.append(n * oy * oy)
                sxy4.append(oy * (s + e) * n)
            else:
                area[label] += n
                if s < min_x[label]:
                    min_x[label] = s
                if e - 1 > max_x[label]:
                    max_x[label] = e - 1
                max_y[label] = y
                sx2[label] += (s + e) * n
                sy2[label] += n * oy
                sxx4[label] += odd_squares[e] - odd_squares[s]
                syy4[label] += n * oy * oy
                sxy4[label] += oy * (s + e) * n
            current.append((s, e, label))
        previous = current
    roots = []
    for label in range(1, len(parent)):
        root = find(label)
        if root == label:
            roots.append(label)
            continue
        area[root] += area[label]
        if min_x[label] < min_x[root]:
            min_x[root] = min_x[label]
        if max_x[label] > max_x[root]:
            max_x[root] = max_x[label]
        if min_y[label] < min_y[root]:
            min_y[root] = min_y[label]
        if max_y[label] > max_y[root]:
            max_y[root] = max_y[label]
        sx2[root] += sx2[label]
        sy2[root] += sy2[label]
        sxx4[root] += sxx4[label]
        syy4[root] += syy4[label]
        sxy4[root] += sxy4[label]
    return [
        Component(
            area[r],
            min_x[r],
            max_x[r],
            min_y[r],
            max_y[r],
            sx2[r] / 2,
            sy2[r] / 2,
            sxx4[r] / 4,
            syy4[r] / 4,
            sxy4[r] / 4,
        )
        for r in roots
    ]


def label_components(mask: Sequence, width: int, height: int) -> List[Component]:
    """Label 4-connected components of a row-major mask; returns those with area > 0.

    ``mask`` holds ``width * height`` truthy/falsy values (for example the
    output of :func:`adaptive_binarize`).
    """
    if len(mask) != width * height:
        raise ValueError("mask length does not match width * height")
    if isinstance(mask, (bytes, bytearray)):
        flat = bytes(mask).translate(_NONZERO)
    else:
        flat = bytes(1 if value else 0 for value in mask)
    rows = [flat[y * width : (y + 1) * width] for y in range(height)]
    return _label_rows(rows, width)


_NONZERO = bytes([0] + [1] * 255)


def blossoms(components: Sequence[Component]) -> List[Finder]:
    """Detect blossom finders: large, round, isolated blobs."""
    others = []
    for index, c in enumerate(components):
        if c.area >= 8:
            cx, cy = c.centroid()
            others.append((index, c.area, cx, cy))
    found = []
    for index, blob in enumerate(components):
        width = blob.width()
        height = blob.height()
        size = width if width > height else height
        if size < 14.0 or blob.area < 100:
            continue
        fill = blob.area / (width * height)
        if not 0.45 <= fill <= 0.9 or blob.axis_ratio() < 0.5:
            continue
        x, y = blob.centroid()
        # isolation: nothing else of substance close by
        smallest = 0.015 * blob.area
        reach = 0.8 * size
        crowded = False
        for other, other_area, ox, oy in others:
            if other == index or other_area < smallest:
                continue
            ex = ox - x
            ey = oy - y
            if math.sqrt(ex * ex + ey * ey) < reach:
                crowded = True
                break
        if not crowded:
            found.append(Finder(x, y, size))
    return found


def _cross(o: Tuple[float, float], a: Tuple[float, float], b: Tuple[float, float]) -> float:
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def _order_clockwise(quad: Sequence[Finder]) -> Optional[Tuple[Finder, ...]]:
    """Order four finders clockwise (as displayed, ``y`` down) from the top-left one."""
    cx = (((quad[0].x + quad[1].x) + quad[2].x) + quad[3].x) / 4.0
    cy = (((quad[0].y + quad[1].y) + quad[2].y) + quad[3].y) / 4.0
    ordered = sorted(quad, key=lambda f: total_key(math.atan2(f.y - cy, f.x - cx)))
    # atan2 grows clockwise on screen because y points down; verify convexity
    for i in range(4):
        o = (ordered[i].x, ordered[i].y)
        a = (ordered[(i + 1) % 4].x, ordered[(i + 1) % 4].y)
        b = (ordered[(i + 2) % 4].x, ordered[(i + 2) % 4].y)
        if _cross(o, a, b) <= 0.0:
            return None
    start = min(range(4), key=lambda i: total_key(ordered[i].x + ordered[i].y))
    return tuple(ordered[(start + i) % 4] for i in range(4))


def _fmin(values: Sequence[float]) -> float:
    """``fold(f64::MAX, f64::min)``: NaN values are ignored."""
    out = F64_MAX
    for v in values:
        if v < out:
            out = v
    return out


def _fmax(values: Sequence[float]) -> float:
    """``fold(0.0, f64::max)``: NaN values are ignored."""
    out = 0.0
    for v in values:
        if v > out:
            out = v
    return out


def _select_quad_from(finders: Sequence[Finder]) -> Optional[Tuple[Finder, ...]]:
    if len(finders) < 4:
        return None
    # Largest first (ties keep discovery order) so that clutter in a busy scene
    # cannot push the real finders out of the ten candidates that are combined.
    order = sorted(range(len(finders)), key=lambda i: (-total_key(finders[i].size), i))
    ranked = [finders[i] for i in order[:10]]
    best: Optional[Tuple[float, Tuple[Finder, ...]]] = None
    n = len(ranked)
    for a in range(n):
        for b in range(a + 1, n):
            for c in range(b + 1, n):
                for d in range(c + 1, n):
                    group = (ranked[a], ranked[b], ranked[c], ranked[d])
                    sizes = [f.size for f in group]
                    smin = _fmin(sizes)
                    smax = _fmax(sizes)
                    if ieee_div(smax, smin) > 1.9:
                        continue
                    quad = _order_clockwise(group)
                    if quad is None:
                        continue
                    sides = []
                    for i in range(4):
                        p = quad[i]
                        q = quad[(i + 1) % 4]
                        ex = p.x - q.x
                        ey = p.y - q.y
                        sides.append(math.sqrt(ex * ex + ey * ey))
                    lmin = _fmin(sides)
                    lmax = _fmax(sides)
                    mean_size = (((sizes[0] + sizes[1]) + sizes[2]) + sizes[3]) / 4.0
                    # canvas geometry: side / finder diameter = 880 / 120
                    ratio = ieee_div(
                        (((sides[0] + sides[1]) + sides[2]) + sides[3]) / 4.0, mean_size
                    )
                    if ieee_div(lmax, lmin) > 2.6 or not 4.8 <= ratio <= 10.5:
                        continue
                    score = (
                        (ieee_div(smax, smin) - 1.0)
                        + (ieee_div(lmax, lmin) - 1.0)
                        + abs((ratio - 7.33) / 7.33)
                    )
                    if best is None or score < best[0]:
                        best = (score, quad)
    return None if best is None else best[1]


def select_quad(finders: Sequence[Finder]) -> Optional[Tuple[Finder, ...]]:
    """Choose four finders that look like the corners of one code.

    Lit tiles and merged dots form blob candidates too, so the largest size
    class is tried first: the corner finders are always the biggest isolated
    round blobs in view. Within a class the candidates are ranked by size
    (largest first, ties in discovery order) and at most ten are combined.
    """
    largest = _fmax([f.size for f in finders])
    strong = [f for f in finders if f.size >= 0.55 * largest]
    quad = _select_quad_from(strong)
    if quad is None:
        quad = _select_quad_from(finders)
    return quad


def refine_center(image: Luma, finder: Finder) -> Finder:
    """Sharpen a finder centre with an intensity-weighted centroid."""
    if not (math.isfinite(finder.x) and math.isfinite(finder.y) and math.isfinite(finder.size)):
        return finder
    half = finder.size * 0.5
    radius = int(math.ceil(half))
    cx = int(math.floor(finder.x))
    cy = int(math.floor(finder.y))
    w = image.width
    h = image.height
    data = image.data
    fx = finder.x
    fy = finder.y
    samples = []
    for dy in range(-radius, radius + 1):
        y = cy + dy
        if y < 0 or y >= h:
            continue
        py = y + 0.5
        ey = py - fy
        ey2 = ey * ey
        base = y * w
        for dx in range(-radius, radius + 1):
            x = cx + dx
            if x < 0 or x >= w:
                continue
            px = x + 0.5
            ex = px - fx
            if math.sqrt(ex * ex + ey2) <= half:
                samples.append((px, py, float(data[base + x])))
    floor = F64_MAX
    peak = 0.0
    for _, _, v in samples:
        if v < floor:
            floor = v
        if v > peak:
            peak = v
    if peak - floor < 20.0:
        return finder
    threshold = floor + 0.5 * (peak - floor)
    sw = 0.0
    sx = 0.0
    sy = 0.0
    for px, py, v in samples:
        weight = v - threshold
        if not weight > 0.0:
            weight = 0.0
        sw += weight
        sx += weight * px
        sy += weight * py
    if sw <= 0.0:
        return finder
    return Finder(sx / sw, sy / sw, finder.size)


def _locate_with(threshold: _Threshold, image: Luma) -> Optional[Tuple[Finder, ...]]:
    for sensitivity in SENSITIVITIES:
        components = _label_rows(threshold.mask_rows(sensitivity), image.width)
        quad = select_quad(blossoms(components))
        if quad is not None:
            return tuple(refine_center(image, f) for f in quad)
    return None


def locate(image: Luma) -> Optional[Tuple[Finder, ...]]:
    """Locate the four finders of a code, clockwise from the top-left one.

    Progressively stricter thresholds are tried so blurred rings still separate
    from their cores. Returns ``None`` when no plausible quadrilateral exists.
    """
    if image.width == 0 or image.height == 0:
        return None
    return _locate_with(_Threshold(image), image)
