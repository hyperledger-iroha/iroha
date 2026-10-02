# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The sixteen katakana of lane ``K``.

A tile's glyph is a four-bit symbol. The alphabet is the subset of the Iroha
ordering ``イロハニホヘト...ス`` whose members stay most distinguishable after
camera blur: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol ``0`` is
``イ`` and symbol ``15`` is ``ン``.

Glyphs are defined as stroke polylines on a 32x32 grid (``y`` grows downward)
drawn with round caps and joins. The strokes are the rendering definition;
:data:`TEMPLATES` is the derived, byte-exact matching table that every decoder
embeds, so classification never depends on a rasteriser.
"""

from __future__ import annotations

import math
from typing import Sequence, Tuple

__all__ = [
    "GLYPH_COUNT",
    "GLYPH_GRID",
    "STROKE_WIDTH",
    "TEMPLATE_N",
    "GLYPH_CHARS",
    "STROKES",
    "TEMPLATES",
    "is_inked",
    "generate_templates",
]

#: Number of glyphs (symbols) in the alphabet.
GLYPH_COUNT = 16
#: Side of the glyph design grid.
GLYPH_GRID = 32.0
#: Stroke width on the design grid.
STROKE_WIDTH = 6.5
#: Side of the matching template grid.
TEMPLATE_N = 8

#: The sixteen characters in symbol order.
GLYPH_CHARS = "イロハニヘトワカレムノケアヒスン"

Point = Tuple[float, float]

#: Stroke polylines for every glyph, in symbol order, on the 32x32 design grid.
STROKES: Tuple[Tuple[Tuple[Point, ...], ...], ...] = (
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
    (
        ((6.0, 5.0), (24.0, 5.0), (17.0, 15.0), (6.0, 27.0)),
        ((13.0, 15.0), (28.0, 28.0)),
    ),
    # ン
    (((6.0, 7.0), (12.0, 13.0)), ((6.0, 25.0), (14.0, 27.0), (27.0, 9.0))),
)

#: Area-coverage templates (``0..=255``) of the glyph alphabet, row-major 8x8.
#: Generated from :data:`STROKES` by :func:`generate_templates`; the tests
#: regenerate and compare this table.
TEMPLATES: Tuple[Tuple[int, ...], ...] = (
    # イ
    (
          0,   0,   0,   0,  55, 195,  37,   0,
          0,   0,   0,  55, 240, 240,  37,   0,
          0,   0,  55, 240, 255, 142,   0,   0,
          0,  55, 240, 245, 255, 143,   0,   0,
          0, 195, 240,  71, 255, 143,   0,   0,
          0,  37,  37,  16, 255, 143,   0,   0,
          0,   0,   0,  16, 255, 143,   0,   0,
          0,   0,   0,   8, 238, 119,   0,   0,
    ),
    # ロ
    (
          3,  74,  80,  80,  80,  80,  74,   3,
         74, 255, 255, 255, 255, 255, 255,  74,
         80, 255, 134,  80,  80, 134, 255,  80,
         80, 255,  80,   0,   0,  80, 255,  80,
         80, 255,  80,   0,   0,  80, 255,  80,
         80, 255, 134,  80,  80, 134, 255,  80,
         74, 255, 255, 255, 255, 255, 255,  74,
          3,  74,  80,  80,  80,  80,  74,   3,
    ),
    # ハ
    (
          0,   0,   3,  68,   3,   0,   0,   0,
          0,   0,  94, 255, 123,   3,   0,   0,
          0,   0, 191, 255, 255, 108,   0,   0,
          0,  35, 254, 161, 221, 230,  12,   0,
          0, 130, 255,  58,  93, 255, 123,   0,
          2, 225, 215,   0,   2, 210, 239,  18,
         63, 255, 119,   0,   0,  78, 255, 124,
         17, 131,  17,   0,   0,   0, 119,  47,
    ),
    # ニ
    (
          0,   0,   0,   0,   0,   0,   0,   0,
          0,  37,  80,  80,  80,  80,  37,   0,
          0, 195, 255, 255, 255, 255, 195,   0,
          0,  37,  80,  80,  80,  80,  37,   0,
          0,   0,   0,   0,   0,   0,   0,   0,
        134, 207, 207, 207, 207, 207, 207, 134,
        134, 207, 207, 207, 207, 207, 207, 134,
          0,   0,   0,   0,   0,   0,   0,   0,
    ),
    # ヘ
    (
          0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,  37,  37,   0,   0,   0,   0,
          0,  55, 240, 244,  82,   0,   0,   0,
         55, 240, 240, 225, 254, 126,   1,   0,
        239, 240,  55,  22, 194, 255, 169,  10,
        119,  51,   0,   0,   6, 155, 255, 204,
          0,   0,   0,   0,   0,   0, 110, 182,
          0,   0,   0,   0,   0,   0,   0,   0,
    ),
    # ト
    (
          0,   8, 238, 119,   0,   0,   0,   0,
          0,  16, 255, 143,   0,   0,   0,   0,
          0,  16, 255, 156,   0,   0,   0,   0,
          0,  16, 255, 255, 188,  53,   0,   0,
          0,  16, 255, 224, 249, 254, 171,  17,
          0,  16, 255, 143,  33, 162, 251,  57,
          0,  16, 255, 143,   0,   0,   8,   0,
          0,   8, 238, 119,   0,   0,   0,   0,
    ),
    # ワ
    (
          0,   0,   0,   0,   0,   0,   0,   0,
          4, 182, 207, 207, 207, 207, 182,   4,
         16, 255, 234, 207, 207, 243, 247,   4,
         16, 255, 143,   0,   0, 216, 204,   0,
         16, 255, 143,   0,  81, 252, 158,   0,
          8, 238, 123, 138, 254, 226,  48,   0,
          0,  25, 193, 255, 185,  20,   0,   0,
          0,  57, 251, 128,   3,   0,   0,   0,
    ),
    # カ
    (
          0,   0,  57, 251,  57,   0,   0,   0,
          0,   0,  80, 255,  80,   0,   0,   0,
        134, 207, 222, 255, 222, 207, 182,   4,
        134, 207, 224, 255, 222, 234, 255,  16,
          0,   0, 167, 253,  40, 143, 255,  16,
          0,  42, 253, 167,   0, 172, 255,  16,
          0, 165, 253,  42,  90, 255, 174,   0,
          0, 134, 136,   0,  83, 182,  13,   0,
    ),
    # レ
    (
          0,  83, 182,   4,   0,   0,   0,   0,
          0, 143, 255,  16,   0,  19, 183,  83,
          0, 143, 255,  16,  15, 200, 254,  87,
          0, 143, 255,  26, 190, 255, 115,   0,
          0, 143, 255, 193, 255, 128,   0,   0,
          0, 143, 255, 255, 140,   0,   0,   0,
          0, 143, 255, 152,   1,   0,   0,   0,
          0,  47, 120,   3,   0,   0,   0,   0,
    ),
    # ム
    (
          0,   0,   0,  85, 182,   4,   0,   0,
          0,   0,   9, 229, 225,   4,   0,   0,
          0,   0, 117, 255,  96,   0,   0,   0,
          0,  16, 235, 213,  87, 183,  15,   0,
          0, 129, 255,  83,  90, 255, 182,   3,
          8, 243, 255, 249, 237, 252, 255, 101,
          0, 119, 153, 165, 177, 191, 205,  83,
          0,   0,   0,   0,   0,   0,   0,   0,
    ),
    # ノ
    (
          0,   0,   0,   0,  38, 195,  37,   0,
          0,   0,   0,   0, 150, 255,  43,   0,
          0,   0,   0,  14, 242, 192,   0,   0,
          0,   0,   0, 113, 255,  87,   0,   0,
          0,   0,  30, 241, 218,   5,   0,   0,
          0,   1, 185, 253,  61,   0,   0,   0,
          0,  95, 255, 143,   0,   0,   0,   0,
          0,  83, 182,  10,   0,   0,   0,   0,
    ),
    # ケ
    (
          0,   0, 142, 238,   8,   0,   0,   0,
          0,  70, 254, 182,   0,   0,   0,   0,
         18, 228, 255, 222, 207, 207, 207,  83,
         57, 251, 206, 225, 255, 223, 207,  83,
          0,   8,   0, 140, 255,  38,   0,   0,
          0,   0,  55, 240, 216,   0,   0,   0,
          0,  51, 240, 240,  55,   0,   0,   0,
          0, 119, 239,  55,   0,   0,   0,   0,
    ),
    # ア
    (
          0,   0,   0,   0,   0,   0,   0,   0,
         17,  80,  80,  80,  80,  80,  74,   3,
        131, 255, 255, 255, 255, 255, 255,  71,
         17,  80,  91, 255, 178, 161, 255,  50,
          0,   0,  16, 255, 164, 211, 253,  17,
          0,   0, 139, 255, 255, 254, 105,   0,
          0, 106, 255, 183, 182, 101,   0,   0,
          0, 182, 205,  13,   0,   0,   0,   0,
    ),
    # ヒ
    (
          0,  17, 131,  17,   0,   0,   0,   0,
          0,  80, 255,  80,   0,   0,   0,   0,
          0,  80, 255, 134,  80,  80,  57,   0,
          0,  80, 255, 255, 255, 255, 251,   8,
          0,  80, 255, 134,  80,  80,  57,   0,
          0,  80, 255, 134,  80,  80,  74,   3,
          0,  74, 255, 255, 255, 255, 255,  68,
          0,   3,  74,  80,  80,  80,  74,   3,
    ),
    # ス
    (
         17, 137, 143, 143, 143, 143,  83,   0,
         57, 253, 255, 255, 255, 255, 185,   0,
          0,  12,  16,  32, 220, 245,  40,   0,
          0,   0, 119, 253, 255, 107,   0,   0,
          0,   0, 156, 255, 254,  98,   0,   0,
          0, 116, 255, 189, 215, 255, 130,   1,
         57, 254, 200,  11,  17, 192, 255, 145,
         17, 131,  21,   0,   0,   6, 152, 134,
    ),
    # ン
    (
          0,   8,   0,   0,   0,   0,   0,   0,
         57, 251, 105,   0,   0,   1, 119,  47,
         17, 210, 254, 101,   0, 111, 255, 121,
          0,  21, 209, 182,  47, 248, 209,   8,
          0,   0,   4,  14, 214, 246,  42,   0,
         17, 133,  88, 158, 255, 104,   0,   0,
         57, 253, 255, 255, 174,   0,   0,   0,
          0,  24,  88, 133,  18,   0,   0,   0,
    ),
)  # fmt: skip


def segment_distance(px: float, py: float, a: Point, b: Point) -> float:
    """Distance from point ``(px, py)`` to the segment ``a``-``b``."""
    ax, ay = a
    bx, by = b
    dx = bx - ax
    dy = by - ay
    length_sq = dx * dx + dy * dy
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
    return math.sqrt(ex * ex + ey * ey)


def _segments(glyph: int) -> Sequence[Tuple[Point, Point]]:
    return [(stroke[i], stroke[i + 1]) for stroke in STROKES[glyph] for i in range(len(stroke) - 1)]


def is_inked(glyph: int, x: float, y: float) -> bool:
    """Return whether design-grid point ``(x, y)`` is inked in ``glyph``."""
    radius = STROKE_WIDTH / 2.0
    for a, b in _segments(glyph):
        if segment_distance(x, y, a, b) <= radius:
            return True
    return False


def generate_templates() -> Tuple[Tuple[int, ...], ...]:
    """Derive the matching templates from the stroke definitions.

    Each of the 8x8 cells holds the inked fraction of its 4x4 design cells,
    sampled on a 16x16 grid and scaled to ``0..=255``.
    """
    supersample = 16
    cell = GLYPH_GRID / TEMPLATE_N
    radius = STROKE_WIDTH / 2.0
    out = []
    for glyph in range(GLYPH_COUNT):
        segments = _segments(glyph)
        table = []
        for v in range(TEMPLATE_N):
            for u in range(TEMPLATE_N):
                inked = 0
                for sy in range(supersample):
                    y = (v + (sy + 0.5) / supersample) * cell
                    for sx in range(supersample):
                        x = (u + (sx + 0.5) / supersample) * cell
                        for a, b in segments:
                            if segment_distance(x, y, a, b) <= radius:
                                inked += 1
                                break
                coverage = inked / (supersample * supersample)
                table.append(int(math.floor(coverage * 255.0 + 0.5)))
        out.append(tuple(table))
    return tuple(out)
