# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Minimal 8-bit pixel buffers and plane homographies.

Pixel ``(i, j)`` covers ``[i, i+1) x [j, j+1)`` and its centre is at
``(i + 0.5, j + 0.5)``. All homographies in this package map into these
pixel-edge coordinates.

Floating-point code here follows the Rust reference operation by operation in
IEEE double precision, so decoders on every platform sample the same values.
"""

from __future__ import annotations

import math
from itertools import repeat
from operator import add, floordiv, mul
from typing import Optional, Sequence, Tuple

from ._numeric import NAN as _NAN
from ._numeric import ieee_div as _ieee_div
from ._numeric import total_key as _total_key

__all__ = ["Luma", "Rgb", "Homography"]


class Luma:
    """A single-channel 8-bit image (camera luma plane).

    ``data`` holds ``width * height`` row-major samples.
    """

    __slots__ = ("width", "height", "data")

    def __init__(self, width: int, height: int, data: Optional[bytes] = None) -> None:
        if width < 0 or height < 0:
            raise ValueError("image dimensions must not be negative")
        if data is None:
            data = bytes(width * height)
        elif not isinstance(data, (bytes, bytearray)):
            data = bytes(data)
        if len(data) != width * height:
            raise ValueError("luma buffer length does not match width * height")
        self.width = width
        self.height = height
        self.data = data

    @classmethod
    def from_strided(cls, width: int, height: int, stride: int, plane: bytes) -> "Luma":
        """Copy a strided plane (for example the Y plane of an NV21 camera frame).

        Raises :class:`ValueError` when the stride is shorter than a row or the
        plane is too small.
        """
        if width < 0 or height < 1 or stride < width:
            raise ValueError("invalid strided plane geometry")
        view = plane if isinstance(plane, (bytes, bytearray)) else bytes(plane)
        if len(view) < stride * (height - 1) + width:
            raise ValueError("strided plane is too small")
        if stride == width:
            return cls(width, height, bytes(view[: width * height]))
        rows = [bytes(view[row * stride : row * stride + width]) for row in range(height)]
        return cls(width, height, b"".join(rows))

    def at(self, x: int, y: int) -> int:
        """Read pixel ``(x, y)``."""
        return self.data[y * self.width + x]

    def sample(self, x: float, y: float) -> float:
        """Bilinear sample at continuous pixel-edge coordinates, clamped to the border."""
        w = self.width
        fx = x - 0.5
        fy = y - 0.5
        if fx != fx or fy != fy:
            return _NAN
        wm1 = w - 1
        hm1 = self.height - 1
        if fx < 0.0:
            fx = 0.0
        elif fx > wm1:
            fx = float(wm1)
        if fy < 0.0:
            fy = 0.0
        elif fy > hm1:
            fy = float(hm1)
        x0 = int(fx)
        y0 = int(fy)
        x1 = x0 + 1 if x0 < wm1 else wm1
        y1 = y0 + 1 if y0 < hm1 else hm1
        tx = fx - x0
        ty = fy - y0
        data = self.data
        r0 = y0 * w
        r1 = y1 * w
        top = data[r0 + x0] * (1.0 - tx) + data[r0 + x1] * tx
        bottom = data[r1 + x0] * (1.0 - tx) + data[r1 + x1] * tx
        return top * (1.0 - ty) + bottom * ty

    def mirrored(self) -> "Luma":
        """The image flipped left to right (a front-camera preview)."""
        w = self.width
        data = self.data
        return Luma(
            w,
            self.height,
            b"".join(bytes(data[row * w : (row + 1) * w])[::-1] for row in range(self.height)),
        )

    def downscaled(self, factor: int) -> "Luma":
        """Box-filter the image down by an integer ``factor``.

        Output pixel ``(i, j)`` averages the ``factor x factor`` block starting
        at ``(factor * i, factor * j)`` (rounded half up); partial blocks at
        the right and bottom edges are dropped. Pixel-edge coordinates scale
        exactly by ``factor``.
        """
        if factor < 1:
            raise ValueError("downscale factor must be positive")
        if factor == 1:
            return self
        w = self.width
        out_w = w // factor
        out_h = self.height // factor
        n = factor * factor
        half = n // 2
        data = self.data
        rows = []
        for j in range(out_h):
            acc = [0] * out_w
            for k in range(factor):
                start = (j * factor + k) * w
                line = data[start : start + out_w * factor]
                for offset in range(factor):
                    acc = list(map(add, acc, line[offset::factor]))
            rows.append(bytes(map(floordiv, map(add, acc, repeat(half)), repeat(n))))
        return Luma(out_w, out_h, b"".join(rows))

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Luma):
            return NotImplemented
        return (
            self.width == other.width
            and self.height == other.height
            and bytes(self.data) == bytes(other.data)
        )

    def __repr__(self) -> str:
        return f"Luma(width={self.width}, height={self.height})"


class Rgb:
    """An interleaved 8-bit RGB image."""

    __slots__ = ("width", "height", "data")

    def __init__(self, width: int, height: int, data: Optional[bytes] = None) -> None:
        if width < 0 or height < 0:
            raise ValueError("image dimensions must not be negative")
        if data is None:
            data = bytes(width * height * 3)
        elif not isinstance(data, (bytes, bytearray)):
            data = bytes(data)
        if len(data) != width * height * 3:
            raise ValueError("RGB buffer length does not match width * height * 3")
        self.width = width
        self.height = height
        self.data = data

    def to_luma(self) -> Luma:
        """Rec. 601 luma of the image: ``(299 R + 587 G + 114 B + 500) / 1000``."""
        data = self.data
        r = data[0::3]
        g = data[1::3]
        b = data[2::3]
        weighted = map(
            add,
            map(add, map(mul, r, repeat(299)), map(mul, g, repeat(587))),
            map(mul, b, repeat(114)),
        )
        luma = map(floordiv, map(add, weighted, repeat(500)), repeat(1000))
        return Luma(self.width, self.height, bytes(luma))

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Rgb):
            return NotImplemented
        return (
            self.width == other.width
            and self.height == other.height
            and bytes(self.data) == bytes(other.data)
        )

    def __repr__(self) -> str:
        return f"Rgb(width={self.width}, height={self.height})"


Point = Tuple[float, float]


class Homography:
    """A 3x3 projective transform stored row-major in :attr:`m`."""

    __slots__ = ("m",)

    def __init__(self, m: Sequence[float]) -> None:
        values = tuple(float(v) for v in m)
        if len(values) != 9:
            raise ValueError("a homography has nine coefficients")
        self.m = values

    def apply(self, x: float, y: float) -> Point:
        """Map a point."""
        m0, m1, m2, m3, m4, m5, m6, m7, m8 = self.m
        w = m6 * x + m7 * y + m8
        if w == 0.0:
            return _ieee_div(m0 * x + m1 * y + m2, w), _ieee_div(m3 * x + m4 * y + m5, w)
        return (m0 * x + m1 * y + m2) / w, (m3 * x + m4 * y + m5) / w

    def inverse(self) -> Optional["Homography"]:
        """The inverse transform, or ``None`` when singular."""
        m = self.m
        c00 = m[4] * m[8] - m[5] * m[7]
        c01 = m[5] * m[6] - m[3] * m[8]
        c02 = m[3] * m[7] - m[4] * m[6]
        det = m[0] * c00 + m[1] * c01 + m[2] * c02
        if abs(det) < 1e-18:
            return None
        inv = 1.0 / det
        return Homography(
            (
                c00 * inv,
                (m[2] * m[7] - m[1] * m[8]) * inv,
                (m[1] * m[5] - m[2] * m[4]) * inv,
                c01 * inv,
                (m[0] * m[8] - m[2] * m[6]) * inv,
                (m[2] * m[3] - m[0] * m[5]) * inv,
                c02 * inv,
                (m[1] * m[6] - m[0] * m[7]) * inv,
                (m[0] * m[4] - m[1] * m[3]) * inv,
            )
        )

    def compose(self, other: "Homography") -> "Homography":
        """``self * other`` (apply ``other`` first)."""
        a = self.m
        b = other.m
        out = []
        for r in range(3):
            for c in range(3):
                out.append(a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c])
        return Homography(out)

    @classmethod
    def from_points(cls, src: Sequence[Point], dst: Sequence[Point]) -> Optional["Homography"]:
        """Fit the homography taking ``src[i]`` to ``dst[i]``.

        Least squares for more than four pairs, exact for four, using
        Hartley-normalised DLT and an 8x8 Gaussian elimination. Returns ``None``
        for degenerate input.
        """
        if len(src) != len(dst) or len(src) < 4:
            return None
        ts, scale_s = _normalisation(src)
        td, scale_d = _normalisation(dst)
        ata = [[0.0] * 8 for _ in range(8)]
        atb = [0.0] * 8
        for (sx, sy), (dx, dy) in zip(src, dst):
            x = ts[0] * sx + ts[2]
            y = ts[4] * sy + ts[5]
            u = td[0] * dx + td[2]
            v = td[4] * dy + td[5]
            for row, rhs in (
                ((x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y), u),
                ((0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y), v),
            ):
                for i in range(8):
                    ri = row[i]
                    target = ata[i]
                    for j in range(8):
                        target[j] += ri * row[j]
                    atb[i] += ri * rhs
        h = _solve8(ata, atb)
        if h is None:
            return None
        normalised = cls((h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0))
        # H = Td^-1 * Hn * Ts
        td_inv = cls(
            (
                1.0 / scale_d,
                0.0,
                -td[2] / scale_d,
                0.0,
                1.0 / scale_d,
                -td[5] / scale_d,
                0.0,
                0.0,
                1.0,
            )
        )
        ts_mat = cls((scale_s, 0.0, ts[2], 0.0, scale_s, ts[5], 0.0, 0.0, 1.0))
        fitted = td_inv.compose(normalised).compose(ts_mat)
        norm = fitted.m[8]
        if abs(norm) < 1e-15:
            return None
        return cls(tuple(v / norm for v in fitted.m))

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, Homography):
            return NotImplemented
        return self.m == other.m

    def __hash__(self) -> int:
        return hash(self.m)

    def __repr__(self) -> str:
        return "Homography(" + ", ".join(repr(v) for v in self.m) + ")"


#: The identity transform.
Homography.IDENTITY = Homography((1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0))  # type: ignore[attr-defined]


def _normalisation(points: Sequence[Point]) -> Tuple[Tuple[float, ...], float]:
    """Translation to the centroid and isotropic scale to mean distance sqrt(2)."""
    n = float(len(points))
    cx = 0.0
    cy = 0.0
    for px, py in points:
        cx = cx + px / n
        cy = cy + py / n
    total = 0.0
    for px, py in points:
        ex = px - cx
        ey = py - cy
        total += math.sqrt(ex * ex + ey * ey)
    mean = total / n
    scale = math.sqrt(2.0) / mean if mean > 1e-12 else 1.0
    return (scale, 0.0, -scale * cx, 0.0, scale, -scale * cy, 0.0, 0.0, 1.0), scale


def _solve8(a, b) -> Optional[list]:
    """Gaussian elimination with partial pivoting for an 8x8 system."""
    a = [list(row) for row in a]
    b = list(b)
    for col in range(8):
        # `max_by` keeps the last of equal maxima.
        pivot = col
        best = _total_key(abs(a[col][col]))
        for i in range(col + 1, 8):
            key = _total_key(abs(a[i][col]))
            if key >= best:
                best = key
                pivot = i
        if abs(a[pivot][col]) < 1e-14:
            return None
        a[col], a[pivot] = a[pivot], a[col]
        b[col], b[pivot] = b[pivot], b[col]
        pivot_row = a[col]
        diagonal = pivot_row[col]
        for row in range(col + 1, 8):
            target = a[row]
            factor = target[col] / diagonal
            for k in range(col, 8):
                target[k] -= factor * pivot_row[k]
            b[row] -= factor * b[col]
    x = [0.0] * 8
    for row in range(7, -1, -1):
        tail = 0.0
        first = True
        for k in range(row + 1, 8):
            term = a[row][k] * x[k]
            if first:
                tail = term
                first = False
            else:
                tail += term
        x[row] = (b[row] - tail) / a[row][row]
    return x
