# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""PNG and PGM/PPM reading and writing."""

from __future__ import annotations

import random
import struct
import tempfile
import unittest
import zlib
from pathlib import Path

import petal_test_support  # noqa: F401  (puts src on sys.path)

from iroha_petal.image import Luma, Rgb
from iroha_petal.pngio import (
    PNG_SIGNATURE,
    decode_png,
    encode_pgm,
    encode_png,
    image_to_luma,
    png_to_luma,
    pnm_to_luma,
    read_luma,
    write_image,
)


def paeth(a: int, b: int, c: int) -> int:
    p = a + b - c
    pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
    if pa <= pb and pa <= pc:
        return a
    return b if pb <= pc else c


def filtered(pixels: bytes, width: int, height: int, bpp: int, kinds) -> bytes:
    """Apply PNG filter ``kinds[y]`` to every row (an independent encoder)."""
    stride = width * bpp
    out = bytearray()
    prev = bytes(stride)
    for y in range(height):
        row = pixels[y * stride : (y + 1) * stride]
        kind = kinds[y]
        out.append(kind)
        for i in range(stride):
            a = row[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            predictor = (0, a, b, (a + b) // 2, paeth(a, b, c))[kind]
            out.append((row[i] - predictor) & 0xFF)
        prev = row
    return bytes(out)


def chunk(kind: bytes, body: bytes) -> bytes:
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def make_png(width, height, color_type, idat, depth=8, interlace=0, palette=None) -> bytes:
    header = struct.pack(">IIBBBBB", width, height, depth, color_type, 0, 0, interlace)
    body = chunk(b"IHDR", header)
    if palette is not None:
        body += chunk(b"PLTE", palette)
    return PNG_SIGNATURE + body + chunk(b"IDAT", zlib.compress(idat)) + chunk(b"IEND", b"")


class PngTest(unittest.TestCase):
    def test_encodes_a_valid_signature_and_chunks(self) -> None:
        png = encode_png(2, 2, 1, bytes([0, 64, 128, 255]))
        self.assertEqual(png[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(png[12:16], b"IHDR")
        self.assertIn(b"IDAT", png)
        self.assertTrue(png.endswith(b"\xaeB`\x82"))

    def test_encoder_roundtrips_grey_and_rgb(self) -> None:
        rng = random.Random(3)
        grey = bytes(rng.randrange(256) for _ in range(13 * 7))
        self.assertEqual(decode_png(encode_png(13, 7, 1, grey)), (13, 7, 1, grey))
        rgb = bytes(rng.randrange(256) for _ in range(5 * 9 * 3))
        self.assertEqual(decode_png(encode_png(5, 9, 3, rgb)), (5, 9, 3, rgb))
        with self.assertRaises(ValueError):
            encode_png(2, 2, 2, bytes(8))
        with self.assertRaises(ValueError):
            encode_png(2, 2, 1, bytes(3))

    def test_every_filter_type_and_colour_type_decodes(self) -> None:
        rng = random.Random(11)
        width, height = 9, 10
        for color_type, bpp in ((0, 1), (4, 2), (2, 3), (6, 4)):
            pixels = bytes(rng.randrange(256) for _ in range(width * height * bpp))
            for kinds in ([k] * height for k in range(5)):
                png = make_png(
                    width, height, color_type, filtered(pixels, width, height, bpp, kinds)
                )
                self.assertEqual(
                    decode_png(png), (width, height, bpp, pixels), (color_type, kinds[0])
                )
            mixed = [y % 5 for y in range(height)]
            png = make_png(width, height, color_type, filtered(pixels, width, height, bpp, mixed))
            self.assertEqual(decode_png(png)[3], pixels)

    def test_colour_images_convert_to_rec601_luma(self) -> None:
        rgb = Rgb(3, 1, bytes([255, 0, 0, 0, 255, 0, 0, 0, 255]))
        png = encode_png(3, 1, 3, rgb.data)
        self.assertEqual(png_to_luma(png).data, rgb.to_luma().data)
        rgba = make_png(2, 1, 6, b"\x00" + bytes([255, 0, 0, 7, 0, 0, 255, 9]))
        self.assertEqual(png_to_luma(rgba).data, bytes([76, 29]))
        grey_alpha = make_png(2, 1, 4, b"\x00" + bytes([10, 1, 200, 2]))
        self.assertEqual(png_to_luma(grey_alpha).data, bytes([10, 200]))
        palette = bytes([0, 0, 0, 255, 255, 255, 255, 0, 0])
        indexed = make_png(3, 1, 3, b"\x00" + bytes([2, 1, 0]), palette=palette)
        self.assertEqual(decode_png(indexed), (3, 1, 3, bytes([255, 0, 0, 255, 255, 255, 0, 0, 0])))
        self.assertEqual(png_to_luma(indexed).data, bytes([76, 255, 0]))

    def test_unsupported_and_corrupt_files_are_rejected(self) -> None:
        cases = {
            "not a png": b"GIF89a....",
            "16-bit": make_png(1, 1, 0, b"\x00\x00\x00", depth=16),
            "interlaced": make_png(1, 1, 0, b"\x00\x00", interlace=1),
            "bad filter": make_png(1, 1, 0, b"\x07\x00"),
            "truncated data": make_png(4, 4, 0, b"\x00\x00"),
            "palette without PLTE": make_png(1, 1, 3, b"\x00\x00"),
            "palette index": make_png(1, 1, 3, b"\x00\x05", palette=bytes(3)),
        }
        for name, data in cases.items():
            with self.assertRaises(ValueError, msg=name):
                decode_png(data)
        with self.assertRaises(ValueError):
            image_to_luma(b"BM....")


class PnmTest(unittest.TestCase):
    def test_pgm_roundtrip_and_header_comments(self) -> None:
        image = Luma(4, 3, bytes(range(12)))
        self.assertEqual(pnm_to_luma(encode_pgm(image)), image)
        commented = b"P5 # a comment\n4 # width\n3\n255\n" + bytes(range(12))
        self.assertEqual(pnm_to_luma(commented), image)

    def test_sixteen_bit_and_colour_pnm(self) -> None:
        deep = b"P5\n2 1\n1023\n" + struct.pack(">HH", 0, 1023)
        self.assertEqual(pnm_to_luma(deep).data, bytes([0, 255]))
        ppm = b"P6\n3 1\n255\n" + bytes([255, 0, 0, 0, 255, 0, 0, 0, 255])
        self.assertEqual(pnm_to_luma(ppm).data, bytes([76, 150, 29]))
        with self.assertRaises(ValueError):
            pnm_to_luma(b"P5\n4 4\n255\n" + bytes(3))
        with self.assertRaises(ValueError):
            pnm_to_luma(b"P2\n1 1\n255\n0")

    def test_files_roundtrip_through_disk(self) -> None:
        image = Luma(6, 5, bytes((i * 7) & 0xFF for i in range(30)))
        rgb = Rgb(2, 2, bytes(range(12)))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            write_image(root / "a.png", image)
            write_image(root / "b.pgm", image)
            write_image(root / "c.png", rgb)
            write_image(root / "d.pgm", rgb)
            self.assertEqual(read_luma(root / "a.png"), image)
            self.assertEqual(read_luma(root / "b.pgm"), image)
            self.assertEqual(read_luma(root / "c.png"), rgb.to_luma())
            self.assertEqual(read_luma(root / "d.pgm"), rgb.to_luma())


if __name__ == "__main__":
    unittest.main()
