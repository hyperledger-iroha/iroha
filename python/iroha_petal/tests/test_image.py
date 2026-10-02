# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Luma/RGB buffers, bilinear sampling and homographies."""

from __future__ import annotations

import math
import unittest

import petal_test_support  # noqa: F401  (puts src on sys.path)

from iroha_petal.image import Homography, Luma, Rgb


class LumaTest(unittest.TestCase):
    def test_bilinear_sampling_interpolates_between_pixel_centres(self) -> None:
        image = Luma(2, 1, bytes([0, 100]))
        self.assertAlmostEqual(image.sample(0.5, 0.5), 0.0, delta=1e-9)
        self.assertAlmostEqual(image.sample(1.5, 0.5), 100.0, delta=1e-9)
        self.assertAlmostEqual(image.sample(1.0, 0.5), 50.0, delta=1e-9)
        self.assertAlmostEqual(image.sample(-5.0, 9.0), 0.0, delta=1e-9)
        self.assertAlmostEqual(image.sample(math.inf, -math.inf), 100.0, delta=1e-9)
        self.assertTrue(math.isnan(image.sample(math.nan, 0.5)))

    def test_strided_planes_drop_the_padding(self) -> None:
        plane = bytes([1, 2, 9, 9, 3, 4, 9, 9])
        image = Luma.from_strided(2, 2, 4, plane)
        self.assertEqual(image.data, bytes([1, 2, 3, 4]))
        with self.assertRaises(ValueError):
            Luma.from_strided(2, 2, 1, plane)
        with self.assertRaises(ValueError):
            Luma.from_strided(2, 3, 4, plane)
        # the last row needs no padding after it
        self.assertEqual(Luma.from_strided(2, 2, 4, plane[:6]).data, bytes([1, 2, 3, 4]))
        self.assertEqual(Luma.from_strided(2, 2, 4, bytearray(plane)).at(1, 1), 4)

    def test_buffers_must_match_their_size(self) -> None:
        with self.assertRaises(ValueError):
            Luma(3, 3, bytes(8))
        with self.assertRaises(ValueError):
            Rgb(2, 2, bytes(11))
        self.assertEqual(Luma(4, 2).data, bytes(8))

    def test_rgb_luma_uses_rec601_weights(self) -> None:
        rgb = Rgb(3, 1, bytes([255, 0, 0, 0, 255, 0, 0, 0, 255]))
        self.assertEqual(rgb.to_luma().data, bytes([76, 150, 29]))

    def test_mirror_and_box_downscale(self) -> None:
        image = Luma(3, 2, bytes([1, 2, 3, 4, 5, 6]))
        self.assertEqual(image.mirrored().data, bytes([3, 2, 1, 6, 5, 4]))
        big = Luma(5, 4, bytes(range(20)))
        small = big.downscaled(2)
        self.assertEqual((small.width, small.height), (2, 2))
        # (0+1+5+6)/4 = 3, (2+3+7+8)/4 = 5, (10+11+15+16)/4 = 13, (12+13+17+18)/4 = 15
        self.assertEqual(small.data, bytes([3, 5, 13, 15]))
        self.assertIs(big.downscaled(1), big)
        with self.assertRaises(ValueError):
            big.downscaled(0)


class HomographyTest(unittest.TestCase):
    def test_four_points_are_mapped_exactly(self) -> None:
        src = [(0.0, 0.0), (1024.0, 0.0), (1024.0, 1024.0), (0.0, 1024.0)]
        dst = [(103.5, 40.25), (590.0, 70.0), (560.0, 420.0), (80.0, 380.0)]
        h = Homography.from_points(src, dst)
        self.assertIsNotNone(h)
        for (sx, sy), (dx, dy) in zip(src, dst):
            x, y = h.apply(sx, sy)
            self.assertLess(abs(x - dx), 1e-7)
            self.assertLess(abs(y - dy), 1e-7)

    def test_inverse_roundtrips_and_least_squares_averages_noise(self) -> None:
        truth = Homography((0.4, -0.1, 130.0, 0.12, 0.38, 60.0, 1e-4, -2e-5, 1.0))
        src = [(50.0 + 31.0 * (i % 6), 90.0 + 47.0 * (i // 6)) for i in range(30)]
        dst = []
        for i, (x, y) in enumerate(src):
            tx, ty = truth.apply(x, y)
            jitter = 0.05 if i % 2 == 0 else -0.05
            dst.append((tx + jitter, ty - jitter))
        fit = Homography.from_points(src, dst)
        for x, y in src:
            x0, y0 = truth.apply(x, y)
            x1, y1 = fit.apply(x, y)
            self.assertLess(abs(x0 - x1), 0.1)
            self.assertLess(abs(y0 - y1), 0.1)
        inverse = truth.inverse()
        x, y = truth.apply(300.0, 200.0)
        bx, by = inverse.apply(x, y)
        self.assertLess(abs(bx - 300.0), 1e-6)
        self.assertLess(abs(by - 200.0), 1e-6)

    def test_degenerate_inputs_are_rejected(self) -> None:
        p = [(1.0, 1.0)] * 4
        self.assertIsNone(Homography.from_points(p, p))
        self.assertIsNone(Homography.from_points(p[:3], p[:3]))
        self.assertIsNone(Homography((0.0,) * 9).inverse())
        with self.assertRaises(ValueError):
            Homography((1.0, 0.0))

    def test_compose_and_points_at_infinity(self) -> None:
        scale = Homography((2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 1.0))
        shift = Homography((1.0, 0.0, 5.0, 0.0, 1.0, -3.0, 0.0, 0.0, 1.0))
        self.assertEqual(scale.compose(shift).apply(1.0, 1.0), (12.0, -4.0))
        self.assertEqual(Homography.IDENTITY.apply(3.5, -2.0), (3.5, -2.0))
        horizon = Homography((1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 0.0))
        x, y = horizon.apply(0.0, 1.0)
        self.assertTrue(math.isnan(x))
        self.assertEqual(y, math.inf)


if __name__ == "__main__":
    unittest.main()
