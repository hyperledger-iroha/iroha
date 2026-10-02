# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Finder location: thresholding, labelling, blossoms, quads and refinement.

The fast row-wise threshold and run-based labelling are cross-checked against
literal pixel-by-pixel ports of the Rust reference.
"""

from __future__ import annotations

import random
import unittest

from petal_test_support import captures_fixture, luma_of, rendered

from iroha_petal.image import Luma
from iroha_petal.locate import (
    Finder,
    _order_clockwise,
    _select_quad_from,
    adaptive_binarize,
    blossoms,
    label_components,
    locate,
    refine_center,
    select_quad,
)


def naive_binarize(image: Luma, sensitivity: float) -> bytearray:
    """``adaptive_binarize`` exactly as the reference writes it (integral image)."""
    w, h = image.width, image.height
    data = image.data
    integral = [0] * ((w + 1) * (h + 1))
    for y in range(h):
        row = 0
        for x in range(w):
            row += data[y * w + x]
            integral[(y + 1) * (w + 1) + x + 1] = integral[y * (w + 1) + x + 1] + row
    histogram = [0] * 256
    for v in data:
        histogram[v] += 1
    total = float(len(data))

    def percentile(p: float) -> float:
        target = total * p
        seen = 0.0
        for level, count in enumerate(histogram):
            seen += float(count)
            if seen >= target:
                return float(level)
        return 255.0

    low, high = percentile(0.02), percentile(0.995)
    value_range = max(high - low, 8.0)
    radius = min(max(min(w, h) // 8, 12), 64)
    margin = max(sensitivity * value_range, 5.0)
    floor = low + 0.2 * value_range
    mask = bytearray(w * h)
    for y in range(h):
        y0, y1 = max(y - radius, 0), min(y + radius + 1, h)
        for x in range(w):
            x0, x1 = max(x - radius, 0), min(x + radius + 1, w)
            s = (
                integral[y1 * (w + 1) + x1]
                + integral[y0 * (w + 1) + x0]
                - integral[y0 * (w + 1) + x1]
                - integral[y1 * (w + 1) + x0]
            )
            mean = float(s) / float((x1 - x0) * (y1 - y0))
            value = float(data[y * w + x])
            mask[y * w + x] = 1 if value > mean + margin and value > floor else 0
    return mask


def naive_label(mask, w: int, h: int) -> list:
    """``label_components`` exactly as the reference writes it (pixel union-find)."""
    labels = [0] * (w * h)
    parent = [0]

    def find(label: int) -> int:
        while parent[label] != label:
            parent[label] = parent[parent[label]]
            label = parent[label]
        return label

    for y in range(h):
        for x in range(w):
            i = y * w + x
            if not mask[i]:
                continue
            left = labels[i - 1] if x > 0 else 0
            up = labels[i - w] if y > 0 else 0
            if left == 0 and up == 0:
                label = len(parent)
                parent.append(label)
                labels[i] = label
            elif up == 0:
                labels[i] = left
            elif left == 0:
                labels[i] = up
            else:
                a, b = find(left), find(up)
                keep, drop = (a, b) if a < b else (b, a)
                parent[drop] = keep
                labels[i] = keep
    components = [None] * len(parent)
    for y in range(h):
        for x in range(w):
            label = labels[y * w + x]
            if label == 0:
                continue
            root = find(label)
            c = components[root]
            if c is None:
                c = components[root] = [0, x, x, y, y, 0.0, 0.0, 0.0, 0.0, 0.0]
            c[0] += 1
            c[1], c[2] = min(c[1], x), max(c[2], x)
            c[3], c[4] = min(c[3], y), max(c[4], y)
            px, py = x + 0.5, y + 0.5
            c[5] += px
            c[6] += py
            c[7] += px * px
            c[8] += py * py
            c[9] += px * py
    return [tuple(c) for c in components if c is not None]


def as_tuples(components) -> list:
    return [
        (c.area, c.min_x, c.max_x, c.min_y, c.max_y, c.sum_x, c.sum_y, c.sum_xx, c.sum_yy, c.sum_xy)
        for c in components
    ]


def clean_frame() -> Luma:
    # payload [9; 200], kind 1, frame 1, 512 px with 2x2 supersampling
    return rendered(bytes([9] * 200), 1, 1, 512, 2).to_luma()


class LocateTest(unittest.TestCase):
    def test_finds_the_four_corner_blossoms_in_a_clean_render(self) -> None:
        quad = locate(clean_frame())
        self.assertIsNotNone(quad)
        expected = [(36.0, 36.0), (476.0, 36.0), (476.0, 476.0), (36.0, 476.0)]
        for finder, (ex, ey) in zip(quad, expected):
            self.assertLess(abs(finder.x - ex), 1.5, finder)
            self.assertLess(abs(finder.y - ey), 1.5, finder)
            self.assertLess(abs(finder.size - 60.0), 6.0, f"size {finder.size}")

    def test_components_are_labelled_with_correct_geometry(self) -> None:
        mask = [False] * 64
        for y in range(1, 4):
            for x in range(2, 6):
                mask[y * 8 + x] = True
        mask[6 * 8 + 6] = True
        components = label_components(mask, 8, 8)
        self.assertEqual(len(components), 2)
        big = next(c for c in components if c.area == 12)
        self.assertEqual((big.min_x, big.max_x, big.min_y, big.max_y), (2, 5, 1, 3))
        cx, cy = big.centroid()
        self.assertLess(abs(cx - 4.0), 1e-9)
        self.assertLess(abs(cy - 2.5), 1e-9)
        with self.assertRaises(ValueError):
            label_components(mask, 8, 7)

    def test_ordering_is_clockwise_from_the_top_left(self) -> None:
        def f(x: float, y: float) -> Finder:
            return Finder(x, y, 10.0)

        quad = _order_clockwise([f(90.0, 90.0), f(10.0, 12.0), f(88.0, 8.0), f(12.0, 92.0)])
        self.assertEqual((quad[0].x, quad[0].y), (10.0, 12.0))
        self.assertEqual((quad[1].x, quad[1].y), (88.0, 8.0))
        self.assertEqual((quad[2].x, quad[2].y), (90.0, 90.0))
        # a non-convex arrangement is rejected
        self.assertIsNone(
            _order_clockwise([f(0.0, 0.0), f(100.0, 0.0), f(50.0, 10.0), f(50.0, 100.0)])
        )

    def test_the_largest_candidates_win_when_clutter_precedes_them(self) -> None:
        # twelve small decoys discovered before the four real finders
        candidates = [Finder(10.0 + 7.0 * i, 5.0, 18.0) for i in range(12)]
        candidates += [
            Finder(100.0, 100.0, 60.0),
            Finder(700.0, 110.0, 62.0),
            Finder(690.0, 520.0, 58.0),
            Finder(95.0, 510.0, 61.0),
        ]
        quad = select_quad(candidates)
        self.assertIsNotNone(quad, "real finders found")
        self.assertEqual(sorted(int(f.x) for f in quad), [95, 100, 690, 700])
        # the size ranking itself: ten mid-size decoys that are not a code, listed
        # first, must not crowd the four real finders out of the ten combined
        decoys = [Finder(40.0 * i, 900.0 + 3.0 * i, 40.0) for i in range(10)]
        quad = _select_quad_from(decoys + candidates[12:])
        self.assertIsNotNone(quad)
        self.assertEqual(sorted(int(f.x) for f in quad), [95, 100, 690, 700])

    def test_a_blank_image_has_no_finders(self) -> None:
        self.assertIsNone(locate(Luma(200, 200)))
        self.assertIsNone(locate(Luma(0, 0)))

    def test_fast_threshold_matches_the_literal_reference(self) -> None:
        rng = random.Random(1)
        noisy = Luma(97, 61, bytes(rng.randrange(256) for _ in range(97 * 61)))
        capture = luma_of(captures_fixture()["captures"][4])  # small-480p
        for image in (noisy, capture, clean_frame()):
            for sensitivity in (0.12, 0.22, 0.34):
                self.assertEqual(
                    adaptive_binarize(image, sensitivity), naive_binarize(image, sensitivity)
                )

    def test_run_labelling_matches_the_pixel_union_find(self) -> None:
        rng = random.Random(5)
        for trial in range(60):
            w, h = rng.randint(1, 40), rng.randint(1, 40)
            density = rng.random()
            mask = bytearray(1 if rng.random() < density else 0 for _ in range(w * h))
            self.assertEqual(
                as_tuples(label_components(mask, w, h)), naive_label(mask, w, h), f"trial {trial}"
            )
        image = luma_of(captures_fixture()["captures"][3])  # soft-480p
        mask = adaptive_binarize(image, 0.22)
        self.assertEqual(
            as_tuples(label_components(mask, image.width, image.height)),
            naive_label(mask, image.width, image.height),
        )

    def test_blossoms_select_and_refine(self) -> None:
        image = clean_frame()
        mask = adaptive_binarize(image, 0.12)
        found = blossoms(label_components(mask, image.width, image.height))
        quad = select_quad(found)
        self.assertIsNotNone(quad)
        self.assertEqual(len(quad), 4)
        refined = refine_center(image, quad[0])
        self.assertLess(abs(refined.x - 36.0), 1.0)
        self.assertEqual(refined.size, quad[0].size)
        # flat regions and non-finite finders are returned unchanged
        flat = Finder(100.0, 100.0, 20.0)
        self.assertEqual(refine_center(Luma(200, 200), flat), flat)
        odd = Finder(float("nan"), 3.0, 10.0)
        self.assertIs(refine_center(image, odd), odd)
        self.assertIsNone(select_quad(found[:3]))
        self.assertIsNone(select_quad([]))


if __name__ == "__main__":
    unittest.main()
