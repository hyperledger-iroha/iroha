# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Shared helpers for the iroha_petal tests (fixtures, renders, a toy camera)."""

from __future__ import annotations

import base64
import functools
import json
import math
import sys
import zlib
from pathlib import Path

TESTS_DIR = Path(__file__).resolve().parent
PACKAGE_DIR = TESTS_DIR.parent
SRC_DIR = PACKAGE_DIR / "src"
REPO_ROOT = PACKAGE_DIR.parents[1]
FIXTURES = REPO_ROOT / "fixtures" / "petal"

if str(SRC_DIR) not in sys.path:
    sys.path.insert(0, str(SRC_DIR))

from iroha_petal import (  # noqa: E402
    DECODE_ORDER,
    SQUARE_TOP,
    Homography,
    Luma,
    RenderOptions,
    StreamEncoder,
    Xorshift32,
    render_frame,
    render_square,
)


@functools.lru_cache(maxsize=None)
def stream_fixture() -> dict:
    """``fixtures/petal/petal_stream_v5.json``."""
    return json.loads((FIXTURES / "petal_stream_v5.json").read_text(encoding="utf-8"))


@functools.lru_cache(maxsize=None)
def captures_fixture() -> dict:
    """``fixtures/petal/petal_captures_v5.json``."""
    return json.loads((FIXTURES / "petal_captures_v5.json").read_text(encoding="utf-8"))


def luma_of(entry: dict, key: str = "luma_zlib_base64") -> Luma:
    """Inflate a capture's zlib + base64 luma plane (``key`` names the field)."""
    data = zlib.decompress(base64.b64decode(entry[key]))
    return Luma(entry["width"], entry["height"], data)


def truth(entry: dict) -> dict:
    """The recorded data of every lane of a fixture entry, by lane: ``D``, then the four
    field lanes."""
    recorded = [bytes.fromhex(entry["d_data"])]
    recorded += [bytes.fromhex(data) for data in entry["field_data"]]
    return dict(zip(DECODE_ORDER, recorded))


def read_lanes(test, name: str, frame, recorded: dict) -> str:
    """Checks every decoded lane of ``frame`` against the recorded data and returns the
    lane letters."""
    for lane, expected in recorded.items():
        result = frame.lane(lane)
        if result is not None:
            test.assertEqual(result.data, expected, f"{name}: lane {lane.letter} data")
    return frame.lanes


def letters(fields) -> str:
    """The sector digits of the field lanes that decoded."""
    return "".join(str(sector) for sector, result in enumerate(fields) if result is not None)


def payload(length: int, seed: int) -> bytes:
    """``length`` bytes from xorshift32 (top byte of each word), as the Rust tests use."""
    rng = Xorshift32(seed)
    return bytes(rng.next_byte() for _ in range(length))


def decode_test_payload() -> bytes:
    """The 3000-byte payload of the reference decoder tests."""
    return bytes(((i * 2_654_435_761) & 0xFFFFFFFF) >> 11 & 0xFF for i in range(3000))


class Lcg:
    """The tiny 64-bit LCG of the Rust Reed-Solomon tests."""

    def __init__(self, seed: int) -> None:
        self.state = seed

    def next(self) -> int:
        self.state = (self.state * 6_364_136_223_846_793_005 + 1_442_695_040_888_963_407) & (
            (1 << 64) - 1
        )
        return self.state >> 33

    def byte(self) -> int:
        return self.next() & 0xFF

    def below(self, bound: int) -> int:
        return self.next() % bound


@functools.lru_cache(maxsize=None)
def encoder(data: bytes, kind: int) -> StreamEncoder:
    return StreamEncoder(data, kind)


@functools.lru_cache(maxsize=32)
def rendered(data: bytes, kind: int, frame: int, size: int, supersample: int):
    """Render (and cache) one frame of the stream carrying ``data``: the card, ``size``
    pixels wide."""
    return render_frame(
        encoder(data, kind).cells(frame), RenderOptions(size=size, supersample=supersample)
    )


@functools.lru_cache(maxsize=32)
def rendered_square(data: bytes, kind: int, frame: int, size: int, supersample: int):
    """Render (and cache) the bare square of one frame of the stream carrying ``data``,
    ``size`` pixels on a side: for tests that work on squares (pixel positions, locator
    expectations, painting over a blossom)."""
    return render_square(
        encoder(data, kind).cells(frame), RenderOptions(size=size, supersample=supersample)
    )


def _columns(image: Luma) -> list:
    w = image.width
    rows = [image.data[y * w : (y + 1) * w] for y in range(image.height)]
    return list(zip(*rows))  # columns[c][r] = in[r][c]


def rotate90(image: Luma) -> Luma:
    """The reference test's ``rot90``: ``out(x, y) = in(y, h - 1 - x)``."""
    columns = _columns(image)
    rows = [bytes(reversed(columns[y])) for y in range(image.width)]
    return Luma(image.height, image.width, b"".join(rows))


def rotate180(image: Luma) -> Luma:
    """The reference test's ``rot180``: ``out(x, y) = in(w - 1 - x, h - 1 - y)``."""
    return Luma(image.width, image.height, bytes(image.data)[::-1])


def rotate270(image: Luma) -> Luma:
    """The reference test's ``rot270``: ``out(x, y) = in(w - 1 - y, x)``."""
    columns = _columns(image)
    rows = [bytes(columns[image.width - 1 - y]) for y in range(image.width)]
    return Luma(image.height, image.width, b"".join(rows))


def camera_homography(
    width: int, height: int, fill: float, rotation_deg: float, tilt: float = 0.0
) -> Homography:
    """Canvas-to-camera homography: centred, scaled to ``fill`` of the short side,
    rotated, with a little projective tilt."""
    side = fill * min(width, height)
    scale = side / 1024.0
    angle = math.radians(rotation_deg)
    c, s = math.cos(angle) * scale, math.sin(angle) * scale
    # centre the canvas, rotate and scale, then move to the image centre
    m = (
        c,
        -s,
        width / 2.0 - 512.0 * c + 512.0 * s,
        s,
        c,
        height / 2.0 - 512.0 * s - 512.0 * c,
        0.0,
        0.0,
        1.0,
    )
    affine = Homography(m)
    if tilt == 0.0:
        return affine
    # mild perspective about the image centre
    cx, cy = width / 2.0, height / 2.0
    k = tilt / max(width, height)
    shift = Homography((1.0, 0.0, -cx, 0.0, 1.0, -cy, 0.0, 0.0, 1.0))
    back = Homography((1.0, 0.0, cx, 0.0, 1.0, cy, 0.0, 0.0, 1.0))
    project = Homography((1.0, 0.0, 0.0, 0.0, 1.0, 0.0, k, 0.4 * k, 1.0))
    return back.compose(project).compose(shift).compose(affine)


def capture(
    source: Luma, forward: Homography, width: int, height: int, noise_seed: int = 0
) -> Luma:
    """A toy camera: warp a rendered luma frame into a ``width x height`` view.

    ``forward`` maps canvas units (square coordinates) to camera pixels. The source is a
    rendered card (taller than wide: its square starts ``SQUARE_TOP`` units down) or a
    bare square. Every camera pixel samples the source bilinearly (black outside it), then
    a 3-tap blur and a little deterministic noise are applied.
    """
    backward = forward.inverse()
    assert backward is not None
    unit = source.width / 1024.0
    square_top = SQUARE_TOP if source.height > source.width else 0.0
    b0, b1, b2, b3, b4, b5, b6, b7, b8 = backward.m
    sw, sh = source.width, source.height
    data = source.data
    rng = Xorshift32(noise_seed or 1)
    plane = []
    for y in range(height):
        py = y + 0.5
        row = []
        for x in range(width):
            px = x + 0.5
            w = b6 * px + b7 * py + b8
            cx = (b0 * px + b1 * py + b2) / w * unit - 0.5
            cy = ((b3 * px + b4 * py + b5) / w + square_top) * unit - 0.5
            if cx < -0.5 or cy < -0.5 or cx > sw - 0.5 or cy > sh - 0.5:
                row.append(0.0)
                continue
            if cx < 0.0:
                cx = 0.0
            elif cx > sw - 1:
                cx = float(sw - 1)
            if cy < 0.0:
                cy = 0.0
            elif cy > sh - 1:
                cy = float(sh - 1)
            x0 = int(cx)
            y0 = int(cy)
            x1 = min(x0 + 1, sw - 1)
            y1 = min(y0 + 1, sh - 1)
            tx = cx - x0
            ty = cy - y0
            top = data[y0 * sw + x0] * (1 - tx) + data[y0 * sw + x1] * tx
            bottom = data[y1 * sw + x0] * (1 - tx) + data[y1 * sw + x1] * tx
            row.append(top * (1 - ty) + bottom * ty)
        plane.append(row)
    out = bytearray(width * height)
    for y in range(height):
        above = plane[max(y - 1, 0)]
        here = plane[y]
        below = plane[min(y + 1, height - 1)]
        for x in range(width):
            left = max(x - 1, 0)
            right = min(x + 1, width - 1)
            value = (4.0 * here[x] + here[left] + here[right] + above[x] + below[x]) / 8.0
            value = 0.85 * value + 12.0 + (rng.next_u32() % 9) - 4.0
            out[y * width + x] = max(0, min(255, int(value + 0.5)))
    return Luma(width, height, bytes(out))
