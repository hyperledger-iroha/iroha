# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Dependency-free image file I/O for frames and captures.

Reads 8-bit non-interlaced PNG (grey, grey + alpha, RGB, RGBA and palette) and
binary PGM/PPM (``P5``/``P6``), converting colour to Rec. 601 luma exactly like
:meth:`iroha_petal.image.Rgb.to_luma`. Writes grey or RGB PNG and PGM.
"""

from __future__ import annotations

import struct
import zlib
from itertools import accumulate, repeat
from operator import add, and_, floordiv, mul
from pathlib import Path
from typing import Tuple, Union

from .image import Luma, Rgb

__all__ = [
    "PNG_SIGNATURE",
    "encode_png",
    "decode_png",
    "png_to_luma",
    "encode_pgm",
    "pnm_to_luma",
    "image_to_luma",
    "read_luma",
    "write_image",
]

#: The eight-byte PNG file signature.
PNG_SIGNATURE = b"\x89PNG\r\n\x1a\n"

_CHANNELS = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
_MOD256 = bytes(v & 0xFF for v in range(511))


def _chunk(kind: bytes, body: bytes) -> bytes:
    return (
        struct.pack(">I", len(body))
        + kind
        + body
        + struct.pack(">I", zlib.crc32(kind + body) & 0xFFFFFFFF)
    )


def encode_png(width: int, height: int, channels: int, pixels: bytes) -> bytes:
    """Encode ``channels`` (1 = grey, 3 = RGB) interleaved 8-bit pixels as a PNG."""
    if channels not in (1, 3):
        raise ValueError("PNG encoder supports 1 (grey) or 3 (RGB) channels")
    stride = width * channels
    pixels = bytes(pixels)
    if len(pixels) != stride * height:
        raise ValueError("pixel buffer length does not match the image size")
    raw = b"".join(b"\x00" + pixels[row * stride : (row + 1) * stride] for row in range(height))
    header = struct.pack(">IIBBBBB", width, height, 8, 0 if channels == 1 else 2, 0, 0, 0)
    return (
        PNG_SIGNATURE
        + _chunk(b"IHDR", header)
        + _chunk(b"IDAT", zlib.compress(raw, 6))
        + _chunk(b"IEND", b"")
    )


def _paeth(line: bytes, prev: bytes, bpp: int) -> bytearray:
    out = bytearray(line)
    for i in range(len(out)):
        a = out[i - bpp] if i >= bpp else 0
        b = prev[i]
        c = prev[i - bpp] if i >= bpp else 0
        p = a + b - c
        pa = abs(p - a)
        pb = abs(p - b)
        pc = abs(p - c)
        if pa <= pb and pa <= pc:
            predictor = a
        elif pb <= pc:
            predictor = b
        else:
            predictor = c
        out[i] = (out[i] + predictor) & 0xFF
    return out


def _average(line: bytes, prev: bytes, bpp: int) -> bytearray:
    out = bytearray(line)
    for i in range(len(out)):
        left = out[i - bpp] if i >= bpp else 0
        out[i] = (out[i] + ((left + prev[i]) >> 1)) & 0xFF
    return out


def _unfilter(raw: bytes, width: int, height: int, bpp: int) -> bytes:
    stride = width * bpp
    if len(raw) < height * (stride + 1):
        raise ValueError("PNG image data is truncated")
    out = []
    prev = bytes(stride)
    pos = 0
    for _ in range(height):
        kind = raw[pos]
        line = raw[pos + 1 : pos + 1 + stride]
        pos += stride + 1
        if kind == 0:
            current = line
        elif kind == 1:
            row = bytearray(stride)
            for c in range(bpp):
                row[c::bpp] = bytes(map(and_, accumulate(line[c::bpp]), repeat(0xFF)))
            current = bytes(row)
        elif kind == 2:
            current = bytes(map(_MOD256.__getitem__, map(add, line, prev)))
        elif kind == 3:
            current = bytes(_average(line, prev, bpp))
        elif kind == 4:
            current = bytes(_paeth(line, prev, bpp))
        else:
            raise ValueError(f"unknown PNG filter type {kind}")
        out.append(current)
        prev = current
    return b"".join(out)


def decode_png(data: bytes) -> Tuple[int, int, int, bytes]:
    """Decode an 8-bit non-interlaced PNG to ``(width, height, channels, pixels)``.

    Palette images are expanded to RGB (``channels == 3``); other colour types
    keep their channels (grey 1, grey + alpha 2, RGB 3, RGBA 4).
    """
    data = bytes(data)
    if not data.startswith(PNG_SIGNATURE):
        raise ValueError("not a PNG file")
    pos = len(PNG_SIGNATURE)
    header = None
    palette = b""
    idat = []
    while pos + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos : pos + 8])
        body = data[pos + 8 : pos + 8 + length]
        if len(body) != length:
            raise ValueError("truncated PNG chunk")
        pos += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"IDAT":
            idat.append(body)
        elif kind == b"IEND":
            break
    if header is None:
        raise ValueError("PNG has no IHDR chunk")
    width, height, depth, color_type, compression, filter_method, interlace = header
    if depth != 8 or color_type not in _CHANNELS:
        raise ValueError("only 8-bit grey, grey+alpha, RGB, RGBA and palette PNG are supported")
    if compression != 0 or filter_method != 0 or interlace != 0:
        raise ValueError("interlaced or non-standard PNG is not supported")
    bpp = _CHANNELS[color_type]
    pixels = _unfilter(zlib.decompress(b"".join(idat)), width, height, bpp)
    if color_type == 3:
        if not palette:
            raise ValueError("palette PNG without PLTE chunk")
        entries = len(palette) // 3
        table = [palette[3 * i : 3 * i + 3] for i in range(entries)]
        if max(pixels, default=0) >= entries:
            raise ValueError("palette index out of range")
        return width, height, 3, b"".join(map(table.__getitem__, pixels))
    return width, height, bpp, pixels


def _rgb_luma(width: int, height: int, pixels: bytes, channels: int) -> Luma:
    r = pixels[0::channels]
    g = pixels[1::channels]
    b = pixels[2::channels]
    weighted = map(
        add,
        map(add, map(mul, r, repeat(299)), map(mul, g, repeat(587))),
        map(mul, b, repeat(114)),
    )
    luma = map(floordiv, map(add, weighted, repeat(500)), repeat(1000))
    return Luma(width, height, bytes(luma))


def png_to_luma(data: bytes) -> Luma:
    """Decode a PNG and convert it to luma (alpha is ignored)."""
    width, height, channels, pixels = decode_png(data)
    if channels == 1:
        return Luma(width, height, pixels)
    if channels == 2:
        return Luma(width, height, pixels[0::2])
    return _rgb_luma(width, height, pixels, channels)


def encode_pgm(image: Luma) -> bytes:
    """Encode a luma image as binary PGM (``P5``)."""
    return b"P5\n%d %d\n255\n" % (image.width, image.height) + bytes(image.data)


def pnm_to_luma(data: bytes) -> Luma:
    """Decode a binary PGM (``P5``) or PPM (``P6``) to luma."""
    data = bytes(data)
    if data[:2] not in (b"P5", b"P6"):
        raise ValueError("not a binary PGM/PPM file")
    fields = []
    pos = 2
    while len(fields) < 3:
        while pos < len(data) and data[pos : pos + 1].isspace():
            pos += 1
        if pos < len(data) and data[pos : pos + 1] == b"#":
            while pos < len(data) and data[pos : pos + 1] not in (b"\n", b"\r"):
                pos += 1
            continue
        start = pos
        while pos < len(data) and data[pos : pos + 1].isdigit():
            pos += 1
        if start == pos:
            raise ValueError("malformed PGM/PPM header")
        fields.append(int(data[start:pos]))
    width, height, maxval = fields
    if not 0 < maxval < 65536:
        raise ValueError("PGM/PPM maxval out of range")
    pos += 1  # exactly one whitespace byte separates the header from the raster
    channels = 1 if data[:2] == b"P5" else 3
    sample_bytes = 1 if maxval < 256 else 2
    count = width * height * channels
    raster = data[pos : pos + count * sample_bytes]
    if len(raster) != count * sample_bytes:
        raise ValueError("PGM/PPM raster is truncated")
    if sample_bytes == 2:
        values = struct.unpack(">%dH" % count, raster)
    else:
        values = raster
    if maxval != 255:
        half = maxval // 2
        raster = bytes((v * 255 + half) // maxval for v in values)
    if channels == 1:
        return Luma(width, height, raster)
    return _rgb_luma(width, height, raster, 3)


def image_to_luma(data: bytes) -> Luma:
    """Decode PNG, PGM or PPM bytes (sniffed by signature) to luma."""
    if data[:8] == PNG_SIGNATURE:
        return png_to_luma(data)
    if data[:2] in (b"P5", b"P6"):
        return pnm_to_luma(data)
    raise ValueError("unsupported image format (expected PNG, PGM or PPM)")


def read_luma(path: Union[str, Path]) -> Luma:
    """Read a PNG, PGM or PPM file as luma."""
    return image_to_luma(Path(path).read_bytes())


def write_image(path: Union[str, Path], image: Union[Luma, Rgb]) -> None:
    """Write ``image`` as PNG, or as PGM when ``path`` ends in ``.pgm`` (luma only)."""
    path = Path(path)
    if path.suffix.lower() == ".pgm":
        if not isinstance(image, Luma):
            image = image.to_luma()
        path.write_bytes(encode_pgm(image))
        return
    channels = 1 if isinstance(image, Luma) else 3
    path.write_bytes(encode_png(image.width, image.height, channels, image.data))
