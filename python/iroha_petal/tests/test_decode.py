# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Frame decoding on rendered frames, transformed frames and garbage images: the field
read (two-means thresholds, local read, lens-distortion search), hidden corner blossoms,
tracking and the Reed-Solomon erasure schedule."""

from __future__ import annotations

import dataclasses
import math
import unittest
from unittest import mock

from petal_test_support import (
    decode_test_payload,
    encoder,
    rendered_square,
    rotate90,
    rotate180,
    rotate270,
)

from iroha_petal import (
    D_DATA,
    FIELD_CELLS,
    FIELD_DATA,
    FIELD_LANES,
    FINDER_CENTERS,
    Beacon,
    DecodeError,
    DecodeErrorKind,
    DecodeOptions,
    FieldEvidence,
    Finder,
    Homography,
    Lane,
    Luma,
    Pose,
    RsError,
    RsErrorKind,
    StreamAssembler,
    Xorshift32,
    cell_center,
    cell_light,
    decode_frame,
    decode_frame_at,
    encode_lane,
    field_evidence,
    locate,
    merge_fields,
    observed_cells,
    sector_cells,
    track_frame,
)
from iroha_petal._numeric import round_half_away
from iroha_petal.decode import (
    _DISTORTION_STEPS,
    _NOT_SKIPPED,
    _PETAL_SPREAD,
    _canonical_corner,
    _decode_with_erasures,
    _field_levels,
    _fit_pose,
    _local_levels,
    _neighbourhoods,
    _projector,
    _read_field_at,
    _read_sectors,
    _reference_levels,
    _refine_inferred_corner,
    _ring_brightness,
    _two_means,
)
from iroha_petal.layout import BAND_OFFSETS, cell_band_slot

KIND = 2

#: The words the reference accepts in its scan of 40 000 random words per lane (trial number:
#: erasures, rewritten bytes, data): three of lane D, none of a field lane.
SCAN_ACCEPTED_D = {
    9827: (5, 8, "c0300c3dfecddbccf0a64620e94c3fd1828690"),
    12427: (5, 8, "1e2795eb69f8b6bf6dd8da502f87aec4998df8"),
    33444: (5, 8, "a7f57ed4c0da100977229890cfb77829a7a9bc"),
}


def setup(frame: int) -> Luma:
    """The square of frame ``frame`` of the reference decoder-test stream, 768 px, 2x2
    supersampled."""
    return rendered_square(decode_test_payload(), KIND, frame, 768, 2).to_luma()


def lane_data(frame: int):
    return encoder(decode_test_payload(), KIND).lane_data(frame)


def render_pose() -> Pose:
    """The exact canvas-to-pixel pose of the 768-pixel test renders."""
    scale = 768.0 / 1024.0
    return Pose.plain(Homography((scale, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 1.0)))


def hidden_blossom(frame: int, corner: int) -> Luma:
    """A 768-pixel render of frame ``frame`` with the blossom of canonical corner ``corner``
    painted over with background (black RGB is luma 0)."""
    image = setup(frame)
    n = image.width
    scale = n / 1024.0
    cx, cy = FINDER_CENTERS[corner]
    cx, cy = cx * scale, cy * scale
    radius = 75.0 * scale
    limit = radius * radius
    data = bytearray(image.data)
    # the reference tests every pixel; nothing outside this box is within the disc
    reach = int(radius) + 2
    for y in range(max(0, int(cy) - reach), min(n, int(cy) + reach + 1)):
        dy = y + 0.5 - cy
        for x in range(max(0, int(cx) - reach), min(n, int(cx) + reach + 1)):
            dx = x + 0.5 - cx
            if dx * dx + dy * dy <= limit:
                data[y * n + x] = 0
    return Luma(n, n, bytes(data))


def shifted(image: Luma, dx: int, dy: int) -> Luma:
    """Shifts a luma image by whole pixels, filling with black."""
    w, h = image.width, image.height
    data = bytes(image.data)
    blank = bytes(w)
    rows = []
    for y in range(h):
        sy = y - dy
        if not 0 <= sy < h:
            rows.append(blank)
            continue
        row = data[sy * w : (sy + 1) * w]
        if dx >= 0:
            rows.append(bytes(min(dx, w)) + row[: max(w - dx, 0)])
        else:
            rows.append(row[min(-dx, w) :] + bytes(min(-dx, w)))
    return Luma(w, h, b"".join(rows))


def padded(image: Luma, pad: int) -> Luma:
    """Places a luma image in the middle of a larger black frame."""
    width = image.width + 2 * pad
    data = bytes(image.data)
    side = bytes(pad)
    blank = bytes(width)
    rows = [blank] * pad
    for y in range(image.height):
        rows.append(side + data[y * image.width : (y + 1) * image.width] + side)
    rows += [blank] * pad
    return Luma(width, image.height + 2 * pad, b"".join(rows))


def barrel(image: Luma, k: float) -> Luma:
    """The image warped by barrel distortion about its centre, as a cheap lens does: pixel
    ``p`` shows what an undistorted camera saw at ``c + (p - c) / (1 + k r^2 / s^2)``."""
    w, h = image.width, image.height
    cx, cy = w / 2.0, h / 2.0
    side = 0.8 * float(max(w, h))
    scale2 = side * side
    out = bytearray(w * h)
    sample = image.sample
    for y in range(h):
        dy = y + 0.5 - cy
        for x in range(w):
            dx = x + 0.5 - cx
            # invert p_d = c + u (1 + k |u|^2/s^2) for the undistorted offset u
            ux, uy = dx, dy
            for _ in range(8):
                f = 1.0 + k * (ux * ux + uy * uy) / scale2
                ux = dx / f
                uy = dy / f
            out[y * w + x] = int(round_half_away(sample(cx + ux, cy + uy)))
    return Luma(w, h, bytes(out))


def shadow_band(image: Luma, factor: float, first: int, last: int) -> Luma:
    """The image with the columns ``first * width / 100 .. last * width / 100`` dimmed to
    ``factor`` of their level (a shadow band across the code)."""
    width = image.width
    dim = bytes(int(round_half_away(float(v) * factor)) for v in range(256))
    rows = []
    lo, hi = width * first // 100, width * last // 100
    for row in range(image.height):
        line = bytes(image.data[row * width : (row + 1) * width])
        rows.append(line[:lo] + line[lo:hi].translate(dim) + line[hi:])
    return Luma(width, image.height, b"".join(rows))


def field_letters(fields) -> str:
    return "".join(str(s) for s, result in enumerate(fields) if result is not None)


class DecodeTest(unittest.TestCase):
    def assert_error(self, image: Luma, kind: DecodeErrorKind, options=None) -> None:
        with self.assertRaises(DecodeError) as caught:
            decode_frame(image, options)
        self.assertIs(caught.exception.kind, kind)

    def assert_all_lanes(self, decoded, truth, name: str) -> None:
        self.assertEqual(decoded.d.data if decoded.d else None, truth.d, f"{name}: lane D")
        for sector, (result, expected) in enumerate(zip(decoded.fields, truth.fields)):
            self.assertEqual(
                result.data if result else None, expected, f"{name}: field lane {sector}"
            )

    def test_clean_render_decodes_every_lane(self) -> None:
        decoded = decode_frame(setup(5))
        self.assert_all_lanes(decoded, lane_data(5), "clean")
        self.assertEqual((decoded.rotation, decoded.mirrored), (0, False))
        self.assertEqual((decoded.lanes, decoded.lanes_ok), ("D0123", 5))
        # a clean render needs no lens distortion (exactly zero, not merely small)
        self.assertEqual(math.copysign(1.0, decoded.pose.distortion), 1.0)
        self.assertEqual(decoded.pose.distortion, 0.0)
        for result in decoded.fields:
            self.assertEqual((result.erasures, result.corrected), (0, 0))
        self.assertEqual(decoded.lane(Lane.F2), decoded.fields[2])
        self.assertIs(decoded.lane(Lane.D), decoded.d)

    def test_rotated_and_mirrored_renders_decode_with_the_right_orientation(self) -> None:
        image = setup(7)
        truth = lane_data(7)
        cases = (
            ("rot90", rotate90(image), 1, False),
            ("rot180", rotate180(image), 2, False),
            ("rot270", rotate270(image), 3, False),
            # mirrored hypotheses enumerate corners in the opposite direction, so
            # the unrotated mirror reports quarter-turn index 1
            ("mirror", image.mirrored(), 1, True),
        )
        for name, transformed, rotation, mirrored in cases:
            decoded = decode_frame(transformed)
            self.assertEqual(decoded.mirrored, mirrored, name)
            self.assertEqual(decoded.rotation, rotation, f"{name} rotation")
            self.assert_all_lanes(decoded, truth, name)
        with self.assertRaises(DecodeError) as caught:
            decode_frame(image.mirrored(), DecodeOptions(try_mirrored=False))
        self.assertIs(caught.exception.kind, DecodeErrorKind.NO_ORIENTATION)

    def test_the_field_alone_tells_the_orientation_when_the_rings_are_gone(self) -> None:
        # black out the ring band: no gates, no lane D; a wrong quarter turn reads permuted
        # petals that no field codeword accepts
        luma = setup(9)
        n = luma.width
        scale = n / 1024.0
        data = bytearray(luma.data)
        for y in range(n):
            dy = (y + 0.5) / scale - 512.0
            for x in range(n):
                dx = (x + 0.5) / scale - 512.0
                if 392.0 <= math.sqrt(dx * dx + dy * dy) <= 490.0:
                    data[y * n + x] = 0
        # the reference's rotation: out(x, y) = in(y, n - 1 - x)
        rotated = rotate90(Luma(n, n, bytes(data)))
        decoded = decode_frame(rotated)
        self.assertIsNone(decoded.d)
        self.assertEqual(decoded.rotation, 1)
        for result, expected in zip(decoded.fields, lane_data(9).fields):
            self.assertEqual(result.data if result else None, expected)

    def test_corrected_counts_rewritten_bytes_not_just_erasures(self) -> None:
        data = bytes(range(FIELD_DATA))
        lane = Lane.F1
        word = bytearray(encode_lane(lane, data))
        for position in (2, 11, 30, 77):
            word[position] ^= 0x5A
        result = _decode_with_erasures(lane, bytes(word), [1.0] * len(word))
        self.assertIsNotNone(result, "four errors fit")
        self.assertEqual(result.data, data)
        self.assertEqual((result.erasures, result.corrected), (0, 4))
        # with the damaged bytes flagged as least confident, they become erasures
        flagged = [1.0] * len(word)
        for position in (2, 11, 30, 77):
            flagged[position] = 0.0
        result = _decode_with_erasures(lane, bytes(word), flagged)
        self.assertEqual(result.data, data)
        self.assertGreaterEqual(result.corrected, 4)

    def test_blank_frames_report_no_finders(self) -> None:
        self.assert_error(Luma(320, 240), DecodeErrorKind.NO_FINDERS)

    def test_unusable_sizes_are_rejected_without_work(self) -> None:
        unsupported = DecodeErrorKind.UNSUPPORTED_IMAGE
        self.assert_error(Luma(1, 1), unsupported)
        self.assert_error(Luma(47, 400), unsupported)
        self.assert_error(Luma(0, 0), unsupported)
        self.assert_error(Luma(100, 100), unsupported, DecodeOptions(max_pixels=1_000))
        broken = Luma(100, 100)
        broken.data = bytes(5)
        self.assert_error(broken, unsupported)
        identity = Pose.plain(Homography.IDENTITY)
        self.assertIsNone(decode_frame_at(Luma(8, 8), identity))
        # a buffer that does not match the stated size must not reach the sampler
        self.assertIsNone(decode_frame_at(broken, identity))
        decoded = decode_frame(setup(5))
        self.assertIsNone(observed_cells(broken, decoded))
        self.assertIsNone(field_evidence(broken, decoded))
        self.assert_error(Luma(5_000, 3_000), unsupported)

    def test_observed_cells_match_the_drawn_cells_of_a_clean_render(self) -> None:
        image = setup(3)
        decoded = decode_frame(image)
        observed = observed_cells(image, decoded)
        drawn = encoder(decode_test_payload(), KIND).cells(3)
        self.assertEqual((observed.dots, observed.petals), (drawn.dots, drawn.petals))
        # the observed petals are left unturned
        self.assertEqual(set(observed.turns), {0})

    def test_garbage_images_never_crash_or_decode(self) -> None:
        rng = Xorshift32(99)
        for w, h in ((64, 48), (257, 129), (320, 240), (480, 480)):
            for style in range(4):
                if style == 0:  # white noise
                    data = bytes(rng.next_byte() for _ in range(w * h))
                elif style == 1:  # gradient
                    data = bytes((i % w) * 255 // w for i in range(w * h))
                elif style == 2:  # checkerboard
                    data = bytes(
                        230 if (i // w // 8 + i % w // 8) % 2 == 0 else 20 for i in range(w * h)
                    )
                else:  # sparse specks
                    data = bytes(255 if rng.next_u32() % 50 == 0 else 0 for _ in range(w * h))
                with self.assertRaises(DecodeError, msg=f"{w}x{h} style {style}"):
                    decode_frame(Luma(w, h, data))
        # tiny, flat, extreme aspect ratio and saturated inputs
        for image in (
            Luma(48, 48, bytes(range(48)) * 48),
            Luma(48, 48, b"\xff" * (48 * 48)),
            Luma(2_000, 48, bytes(rng.next_byte() for _ in range(2_000 * 48))),
            Luma(48, 600, bytes((i * 7) & 0xFF for i in range(48 * 600))),
        ):
            with self.assertRaises(DecodeError):
                decode_frame(image)

    def test_random_blob_scenes_never_crash(self) -> None:
        # Scenes with several random bright ellipses (some finder-sized) on noise:
        # exercises the locator, quad selection and pose fitting on degenerate layouts.
        rng = Xorshift32(2024)
        for scene in range(60):
            w = 160 + rng.next_u32() % 400
            h = 120 + rng.next_u32() % 300
            data = bytearray(rng.next_u32() % 40 for _ in range(w * h))
            for _ in range(3 + rng.next_u32() % 8):
                cx = float(rng.next_u32() % w)
                cy = float(rng.next_u32() % h)
                rx = 6.0 + rng.next_u32() % 40
                ry = 6.0 + rng.next_u32() % 40
                # the reference tests every pixel; outside this box the test fails
                for y in range(max(0, int(cy - ry) - 1), min(h, int(cy + ry) + 2)):
                    dy = (y - cy) / ry
                    for x in range(max(0, int(cx - rx) - 1), min(w, int(cx + rx) + 2)):
                        dx = (x - cx) / rx
                        if dx * dx + dy * dy <= 1.0:
                            data[y * w + x] = 230
            # must not crash; a lucky layout may locate finders but cannot yield lanes
            try:
                frame = decode_frame(Luma(w, h, bytes(data)))
            except DecodeError:
                continue
            self.assertEqual(frame.lanes_ok, 0, f"scene {scene} produced lane data from blobs")

    def test_a_large_hidden_region_never_reads_wrong_data(self) -> None:
        # the whole bottom-right quarter is gone: rings and petals with it
        image = setup(2)
        n = image.width
        data = bytearray(image.data)
        for y in range(n * 3 // 4, n):
            data[y * n + n * 3 // 4 : y * n + n] = bytes(n - n * 3 // 4)
        truth = lane_data(2)
        try:
            frame = decode_frame(Luma(n, n, bytes(data)))
        except DecodeError:
            return
        for lane in Lane:
            result = frame.lane(lane)
            if result is not None:
                self.assertEqual(result.data, truth.lane(lane), lane)

    def test_known_pose_decoding_reads_every_lane(self) -> None:
        image = setup(4)
        decoded = decode_frame_at(image, render_pose())
        self.assertIsNotNone(decoded)
        self.assertEqual(decoded.lanes, "D0123")
        self.assert_all_lanes(decoded, lane_data(4), "known pose")
        beacon = decoded.beacon()
        self.assertIsInstance(beacon, Beacon)
        self.assertEqual(beacon.meta, encoder(decode_test_payload(), KIND).meta)
        self.assertIsNone(decode_frame_at(Luma(320, 240), render_pose()), "weak reference levels")

    def test_frames_feed_an_assembler(self) -> None:
        stream = encoder(decode_test_payload(), KIND)
        assembler = StreamAssembler()
        for frame in range(stream.systematic_frames()):
            decoded = decode_frame(setup(frame))
            packets = decoded.atom_packets()
            self.assertEqual(sum(len(p.atoms) for p in packets), 20 if frame % 4 == 0 else 21)
            decoded.feed(assembler)
        completed = assembler.take_completed()
        self.assertIsNotNone(completed)
        self.assertEqual(completed.payload, decode_test_payload())

    def test_max_side_downscales_large_inputs_and_keeps_pixel_coordinates(self) -> None:
        image = rendered_square(decode_test_payload(), KIND, 5, 1024, 1).to_luma()
        decoded = decode_frame(image, max_side=512)
        self.assertEqual(decoded.lanes, "D0123")
        for canvas in ((512.0, 512.0), (72.0, 72.0), (952.0, 952.0)):
            x, y = decoded.pose.apply(*canvas)
            self.assertLess(abs(x - canvas[0]), 2.0)
            self.assertLess(abs(y - canvas[1]), 2.0)
        with self.assertRaises(ValueError):
            decode_frame(image, max_side=10)


class ErasureScheduleTest(unittest.TestCase):
    """How many erasures each lane's Reed-Solomon decoding is allowed to try."""

    def test_random_words_are_almost_never_accepted(self) -> None:
        # Reed-Solomon with erasures can accept a word that is not a transmission. Lane D has
        # only 11 parity bytes, so its schedule stops at five erasures; a field lane has 22 and
        # stops at eleven. The reference scans 40 000 xorshift32 words per lane (lane D, then
        # field lane 0 from the same generator), each followed by one byte-valued confidence per
        # position (many ties, so the ranking must be stable), and accepts exactly
        # SCAN_ACCEPTED_D and no word of the field lane. The full scans take a few minutes in
        # CPython and this port reproduces them exactly; this runs the first 10 000 words of
        # lane D and the first 1 000 of the field lane.
        scan = 40_000
        rng = Xorshift32(0x5EED)
        for lane, run in ((Lane.D, 10_000), (Lane.F0, 1_000)):
            length = lane.word_len
            accepted = {}
            for trial in range(run):
                word = bytes(rng.next_byte() for _ in range(length))
                confidence = [float(rng.next_byte()) for _ in range(length)]
                result = _decode_with_erasures(lane, word, confidence)
                if result is not None:
                    accepted[trial] = (result.erasures, result.corrected, result.data.hex())
            if lane is Lane.D:
                expected = {t: v for t, v in SCAN_ACCEPTED_D.items() if t < run}
                # leave the generator where the reference's scan of the field lane starts
                for _ in range((scan - run) * 2 * length):
                    rng.next_u32()
            else:
                expected = {}
            self.assertEqual(accepted, expected, f"lane {lane.name} of {run} random words")

    def test_erasures_stop_at_half_the_parity(self) -> None:
        # damaged bytes: `flagged` of them marked least confident, two more hidden: beyond half
        # the parity the decoder must refuse instead of risking a wrong codeword
        for lane, flagged in ((Lane.D, 7), (Lane.F2, 16)):
            data = bytes(range(lane.data_len))
            word = encode_lane(lane, data)
            damaged = bytearray(word)
            confidence = [1.0] * len(word)
            for position in range(flagged):
                damaged[position] ^= 0xA5
                confidence[position] = 0.0
            damaged[20] ^= 0x3C
            damaged[21] ^= 0x3C
            self.assertIsNone(
                _decode_with_erasures(lane, bytes(damaged), confidence), f"lane {lane.name}"
            )
            # half the parity flagged plus one hidden error stays comfortably repairable
            damaged = bytearray(word)
            confidence = [1.0] * len(damaged)
            for position in range(lane.parity_len // 2):
                damaged[position] ^= 0xA5
                confidence[position] = 0.0
            damaged[24] ^= 0x3C
            result = _decode_with_erasures(lane, bytes(damaged), confidence)
            self.assertIsNotNone(result, f"lane {lane.name} repairable")
            self.assertEqual(result.data, data, f"lane {lane.name}")
            self.assertLessEqual(result.erasures, lane.parity_len // 2, f"lane {lane.name}")

    def test_each_lane_tries_its_own_erasure_counts(self) -> None:
        # integer eighths, quarters, thirds and halves of the parity
        expected = {Lane.D: [0, 1, 2, 3, 5]}
        for lane in FIELD_LANES:
            expected[lane] = [0, 2, 5, 7, 11]
        for lane, counts in expected.items():
            tried = []

            def refuse(lane_, transmitted, erasures, tried=tried):
                tried.append(len(list(erasures)))
                raise RsError(RsErrorKind.UNCORRECTABLE)

            with mock.patch("iroha_petal.decode.decode_lane_counted", side_effect=refuse):
                result = _decode_with_erasures(lane, bytes(lane.word_len), [0.0] * lane.word_len)
            self.assertIsNone(result)
            self.assertEqual(tried, counts, f"lane {lane.name}")

    def test_equal_confidences_are_erased_in_position_order(self) -> None:
        # Ties must be broken by position or ports disagree about which bytes are erased.
        # Three damaged bytes at the front plus four hidden ones fit lane D only if exactly the
        # first three positions are erased (3 erasures + 4 errors = all 11 parity bytes).
        data = bytes(range(D_DATA))
        word = bytearray(encode_lane(Lane.D, data))
        for position in (0, 1, 2, 20, 21, 22, 23):
            word[position] ^= 0x5A
        confidence = [1.0] * len(word)
        result = _decode_with_erasures(Lane.D, bytes(word), confidence)
        self.assertIsNotNone(result, "ties in order")
        self.assertEqual((result.data, result.erasures, result.corrected), (data, 3, 7))


class FieldReadTest(unittest.TestCase):
    """The field read: levels, two-means thresholds, the local read and the distortion
    search."""

    def test_two_means_splits_two_clusters_and_survives_one(self) -> None:
        values = [0.05] * 40 + [0.9] * 60
        threshold = _two_means(values)
        self.assertTrue(0.3 < threshold < 0.6, threshold)
        self.assertLess(abs(_two_means([0.4] * 10) - 0.4), 1e-12)
        self.assertEqual(_two_means([0.0] * 3 + [1.0]), 0.5)
        nan = float("nan")
        self.assertTrue(math.isnan(_two_means([nan, 1.0, 0.0])))

    def test_field_levels_are_five_point_means_normalised_by_the_finders(self) -> None:
        image = setup(5)
        distorted = Pose.fit(
            [(float(x), float(y)) for x, y in FINDER_CENTERS],
            [(x * 0.75 + 3.0, y * 0.75 - 2.0) for x, y in FINDER_CENTERS],
            -0.07,
            image.width,
            image.height,
        )
        for pose in (render_pose(), distorted):
            project = _projector(image, pose)
            reference = _reference_levels(project)
            self.assertIsNotNone(reference)
            levels = _field_levels(project, reference)
            self.assertEqual(len(levels), FIELD_CELLS)
            spread = 0.4 * 4.599999904632568
            self.assertEqual(_PETAL_SPREAD, spread)
            for cell in range(0, FIELD_CELLS, 97):
                x, y = cell_center(cell)
                total = 0.0
                for dx, dy in (
                    (0.0, 0.0),
                    (spread, 0.0),
                    (-spread, 0.0),
                    (0.0, spread),
                    (0.0, -spread),
                ):
                    total += image.sample(*pose.apply(x + dx, y + dy))
                lit, dark = reference.at(x, y)
                self.assertEqual(levels[cell], (total / 5.0 - dark) / (lit - dark), cell)

    def test_neighbourhoods_are_the_cells_within_32_units_in_index_order(self) -> None:
        table = _neighbourhoods()
        self.assertEqual(len(table), FIELD_CELLS)
        for cell in range(0, FIELD_CELLS, 13):
            x, y = cell_center(cell)
            band, _ = cell_band_slot(cell)
            first = BAND_OFFSETS[max(band - 3, 0)]
            last = BAND_OFFSETS[min(band + 4, 34)]
            expected = []
            for other in range(first, last):
                ox, oy = cell_center(other)
                if (ox - x) * (ox - x) + (oy - y) * (oy - y) <= 1024.0:
                    expected.append(other)
            self.assertEqual(table[cell], tuple(expected), cell)
            self.assertIn(cell, table[cell])
        # nothing outside the six neighbouring bands is ever that close
        for cell in (0, 500, 1700, FIELD_CELLS - 1):
            x, y = cell_center(cell)
            near = []
            for other in range(FIELD_CELLS):
                ox, oy = cell_center(other)
                if (ox - x) * (ox - x) + (oy - y) * (oy - y) <= 1024.0:
                    near.append(other)
            self.assertEqual(table[cell], tuple(near))

    def test_local_levels_rescale_each_cell_by_its_neighbourhood(self) -> None:
        levels = [float(cell % 7) for cell in range(FIELD_CELLS)]
        local = _local_levels(levels)
        table = _neighbourhoods()
        for cell in (0, 9, 1234, 3000):
            values = sorted(levels[o] for o in table[cell])
            ends = max(min(5, len(values) // 2), 1)
            low = values[0]
            for v in values[1:ends]:
                low += v
            low /= ends
            high = values[len(values) - ends]
            for v in values[len(values) - ends + 1 :]:
                high += v
            high /= ends
            self.assertEqual(local[cell], (levels[cell] - low) / max(high - low, 0.05))
        # a flat neighbourhood has a span of 0.05
        flat = _local_levels([0.3] * FIELD_CELLS)
        self.assertTrue(all(abs(v) < 1e-12 for v in flat))
        # a gain and an offset common to a neighbourhood cancel
        scaled = _local_levels([0.25 * v + 3.0 for v in levels])
        for cell in (0, 9, 1234, 3000):
            self.assertAlmostEqual(scaled[cell], local[cell], places=9)

    def test_a_shadow_band_is_read_through_the_local_read(self) -> None:
        # a band across the middle of the code in deep shadow: the finder levels cannot
        # describe a step in the light, so the level read loses every field lane, and the
        # local read gets them all back
        luma = shadow_band(setup(5), 0.45, 40, 60)
        project = _projector(luma, render_pose())
        reference = _reference_levels(project)
        self.assertIsNotNone(reference)
        levels = _field_levels(project, reference)
        level_read, _ = _read_sectors(levels, _NOT_SKIPPED)
        self.assertEqual(field_letters(level_read), "")
        local_read, _ = _read_sectors(_local_levels(levels), _NOT_SKIPPED)
        self.assertEqual(field_letters(local_read), "0123")
        decoded = decode_frame(luma)
        truth = lane_data(5)
        self.assertEqual(field_letters(decoded.fields), "0123")
        for result, expected in zip(decoded.fields, truth.fields):
            self.assertEqual(result.data, expected)
        # a known pose reads the same field lanes, local read included
        known = decode_frame_at(luma, render_pose())
        self.assertEqual(known.fields, decoded.fields)

    def test_lens_distortion_is_found_when_the_plain_pose_misses_the_petals(self) -> None:
        # a small, strongly distorted picture: 480 px of code with the corner blossoms far out
        luma = setup(6)
        small = bytearray(640 * 480)
        data = luma.data
        for y in range(384):
            r0 = 2 * y * 768
            r1 = r0 + 768
            for x in range(384):
                total = (
                    data[r0 + 2 * x]
                    + data[r0 + 2 * x + 1]
                    + data[r1 + 2 * x]
                    + data[r1 + 2 * x + 1]
                )
                small[(y + 48) * 640 + x + 128] = (total + 2) // 4
        distorted = barrel(Luma(640, 480, bytes(small)), -0.12)
        quad = locate(distorted)
        self.assertIsNotNone(quad, "four finders")
        # the plain pose misses petals at the edge of the field
        pose = _fit_pose(distorted, [(f.x, f.y) for f in quad], 0.0)
        plain = decode_frame_at(distorted, pose)
        self.assertIsNotNone(plain)
        self.assertIn(None, plain.fields, plain.lanes)
        decoded = decode_frame(distorted)
        self.assertLess(decoded.pose.distortion, 0.0)
        truth = lane_data(6)
        self.assertEqual(decoded.d.data if decoded.d else None, truth.d)
        for result, expected in zip(decoded.fields, truth.fields):
            self.assertEqual(result.data if result else None, expected)

    def test_the_pose_that_lands_on_the_petals_separates_them_best(self) -> None:
        image = setup(5)
        dst = [(x * 0.75, y * 0.75) for x, y in FINDER_CENTERS]
        plain = _read_field_at(image, dst, 0.0, None)
        self.assertEqual(plain.decoded, 4)
        self.assertGreater(plain.separation, 0.0)
        for step in _DISTORTION_STEPS:
            read = _read_field_at(image, dst, step, None)
            self.assertIsNotNone(read)
            self.assertEqual(read.pose.distortion, step)
            # more lanes, then a clearer separation
            self.assertLess(read.separation, plain.separation, step)
            self.assertTrue(plain.better_than(read), step)
            self.assertFalse(read.better_than(read))

    def test_merging_one_capture_reads_the_field_lanes_decode_read(self) -> None:
        for name, image in (("clean", setup(5)), ("hidden corner", hidden_blossom(2, 3))):
            frame = decode_frame(image)
            self.assertNotIn(None, frame.fields, name)
            evidence = field_evidence(image, frame)
            self.assertIsNotNone(evidence, name)
            self.assertEqual(merge_fields([evidence]), frame.fields, name)

    def test_averaging_captures_cancels_their_independent_noise(self) -> None:
        image = setup(5)
        truth = lane_data(5)
        frame = decode_frame(image)
        clean = field_evidence(image, frame)
        rng = Xorshift32(9)
        # uniform noise of +-0.6 of the lit level per cell: about one bit in eight flips in
        # one capture, far more than a field codeword corrects
        captures = [
            FieldEvidence([v + (float(rng.next_byte()) - 127.5) / 213.0 for v in clean.levels])
            for _ in range(4)
        ]
        for capture in captures:
            self.assertEqual(
                merge_fields([capture]),
                (None,) * 4,
                "one noisy capture must not read a field lane, or the test proves nothing",
            )
        merged = merge_fields(captures)
        for sector, (result, expected) in enumerate(zip(merged, truth.fields)):
            self.assertEqual(result.data if result else None, expected, f"sector {sector}")
        # skipped sectors are not read
        skipped = merge_fields(captures, (True, False, True, True))
        self.assertIsNone(skipped[0])
        self.assertIsNotNone(skipped[1])
        self.assertIsNone(skipped[2])

    def test_evidence_needs_a_usable_image_and_a_real_corner(self) -> None:
        image = setup(5)
        frame = decode_frame(image)
        self.assertIsNone(field_evidence(Luma(20, 20), frame))
        self.assertIsNone(field_evidence(image, dataclasses.replace(frame, inferred_corner=7)))
        self.assertEqual(merge_fields([]), (None, None, None, None))
        with self.assertRaises(ValueError):
            FieldEvidence([0.0] * 3)

    def test_light_and_pink_petals_are_thresholded_apart(self) -> None:
        cells = encoder(decode_test_payload(), KIND).cells(5)
        truth = lane_data(5)
        # ideal levels: every sector reads, and every cell sits 0.5 from the threshold
        ideal = [1.0 if lit else 0.0 for lit in cells.petals]
        results, separation = _read_sectors(ideal, _NOT_SKIPPED)
        self.assertEqual([r.data if r else None for r in results], list(truth.fields))
        self.assertEqual(separation, 0.5)
        # skipped sectors are not read, and the separation covers the sectors read
        results, separation = _read_sectors(ideal, (True, False, True, True))
        self.assertEqual(field_letters(results), "1")
        self.assertEqual(separation, 0.5)
        # the light petals of the 天 are much brighter than the pink ones: a dark light
        # petal can be brighter than a lit pink one, so one threshold per codeword fails
        light = [cell_light(cell) for cell in range(FIELD_CELLS)]
        uneven = [
            (1.0 if lit else 0.35) if is_light else (0.3 if lit else 0.0)
            for lit, is_light in zip(cells.petals, light)
        ]
        results, _ = _read_sectors(uneven, _NOT_SKIPPED)
        self.assertEqual([r.data if r else None for r in results], list(truth.fields))
        values = [uneven[c] for c in sector_cells(0)[:840]]
        threshold = _two_means(values)
        lit_pink = [
            cell for cell in sector_cells(0)[:840] if cells.petals[cell] and not light[cell]
        ]
        self.assertTrue(all(uneven[cell] < threshold for cell in lit_pink))


class HiddenCornerTest(unittest.TestCase):
    """Three blossoms forming a corner and the inferred fourth."""

    def test_a_hidden_blossom_is_inferred_and_every_lane_still_reads(self) -> None:
        truth = lane_data(2)
        for corner in range(4):
            decoded = decode_frame(hidden_blossom(2, corner))
            self.assertEqual(decoded.inferred_corner, corner, f"corner {corner}")
            self.assertEqual((decoded.rotation, decoded.mirrored), (0, False), f"corner {corner}")
            self.assertEqual(decoded.d.data if decoded.d else None, truth.d, f"corner {corner}")
            for result, expected in zip(decoded.fields, truth.fields):
                self.assertEqual(result.data if result else None, expected, f"corner {corner}")

    def test_the_inferred_corner_is_reported_in_code_coordinates_when_mirrored(self) -> None:
        # hide the top-right blossom of the code, then mirror the picture: the hidden
        # blossom appears top-left in the image but is still corner 1 of the code
        decoded = decode_frame(hidden_blossom(3, 1).mirrored())
        self.assertTrue(decoded.mirrored)
        self.assertEqual(decoded.inferred_corner, 1)
        self.assertEqual(decoded.d.data if decoded.d else None, lane_data(3).d)

    def test_canonical_corners_follow_the_hypothesis_enumeration(self) -> None:
        # the hypothesis (rotation, mirrored) sends quad corner `(i + r) % 4` (or
        # `(r - i) % 4` mirrored) to canonical corner i; the canonical index of a quad
        # corner inverts that
        for rotation in range(4):
            for i in range(4):
                self.assertEqual(_canonical_corner((i + rotation) % 4, rotation, False), i)
                self.assertEqual(_canonical_corner((rotation + 4 - i) % 4, rotation, True), i)

    def test_inferred_levels_extrapolate_the_other_three_corners(self) -> None:
        image = setup(5)
        project = _projector(image, render_pose())
        seen = _reference_levels(project)
        self.assertIsNotNone(seen)
        for corner in range(4):
            guessed = _reference_levels(project, corner)
            n1, opposite, n2 = (corner + 1) % 4, (corner + 2) % 4, (corner + 3) % 4
            for levels, truth in ((guessed.lit, seen.lit), (guessed.dark, seen.dark)):
                # the three seen corners are measured as before
                for other in (n1, opposite, n2):
                    self.assertEqual(levels[other], truth[other])
                parallelogram = truth[n1] + truth[n2] - truth[opposite]
                low = min(truth[n1], truth[opposite], truth[n2])
                high = max(truth[n1], truth[opposite], truth[n2])
                self.assertEqual(levels[corner], min(max(parallelogram, low), high))
        # a hidden corner on a blank picture still has no contrast at the others
        self.assertIsNone(_reference_levels(_projector(Luma(768, 768), render_pose()), 2))

    def test_the_rings_pull_an_inferred_corner_into_place(self) -> None:
        image = hidden_blossom(2, 2)
        scale = 768.0 / 1024.0
        truth = [Finder(x * scale, y * scale, 90.0) for x, y in FINDER_CENTERS]
        # start 12 pixels off: the ring search pulls the corner most of the way back (the
        # ring dots are 13.5 pixels wide here, so the brightness has a plateau a few pixels
        # wide and the first of its brightest points wins)
        corners = list(truth)
        corners[2] = Finder(truth[2].x + 9.0, truth[2].y - 8.0, 90.0)
        refined = _refine_inferred_corner(image, corners, 2)
        self.assertEqual(refined[:2] + refined[3:], tuple(corners[:2] + corners[3:]))
        self.assertEqual(refined[2].size, 90.0)
        self.assertLess(abs(refined[2].x - truth[2].x), 6.0, refined[2])
        self.assertLess(abs(refined[2].y - truth[2].y), 6.0, refined[2])
        # the refined corner is brighter on the rings than where it started
        points = [(f.x, f.y) for f in corners]
        better = [(f.x, f.y) for f in refined]
        self.assertGreater(_ring_brightness(image, better), _ring_brightness(image, points))
        # a degenerate quad has no pose
        self.assertIsNone(_ring_brightness(image, [(1.0, 1.0)] * 4))

    def test_an_inferred_corner_needs_contrast_too(self) -> None:
        def level_card(levels):
            data = bytearray(1024 * 1024)
            for (cx, cy), (lit, dark) in zip(((72, 72), (952, 72), (952, 952), (72, 952)), levels):
                sx = 1 if cx < 512 else -1
                sy = 1 if cy < 512 else -1
                for px, py, radius, value in (
                    (cx, cy, 30, lit),
                    (cx + sx * 100, cy, 12, dark),
                    (cx, cy + sy * 100, 12, dark),
                ):
                    for y in range(py - radius, py + radius + 1):
                        data[y * 1024 + px - radius : y * 1024 + px + radius + 1] = bytes(
                            [value] * (2 * radius + 1)
                        )
            return _projector(Luma(1024, 1024, bytes(data)), Pose.plain(Homography.IDENTITY))

        # even light: the hidden corner (3) gets levels between the others'
        even = _reference_levels(level_card([(230, 30), (220, 25), (210, 20), (0, 0)]), 3)
        self.assertIsNotNone(even)
        self.assertGreaterEqual(even.lit[3] - even.dark[3], 12.0)
        # the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
        uneven = level_card([(60, 45), (250, 20), (200, 185), (0, 0)])
        self.assertIsNone(_reference_levels(uneven, 3))
        # with every corner seen, the same light is fine
        seen = level_card([(60, 45), (250, 20), (200, 185), (240, 20)])
        self.assertIsNotNone(_reference_levels(seen))


class TrackingTest(unittest.TestCase):
    """Following a code from the previous frame's pose."""

    def test_tracking_follows_a_small_movement_and_gives_up_on_a_jump(self) -> None:
        image = padded(setup(6), 100)
        first = decode_frame(image)
        truth = lane_data(6)
        followed = track_frame(shifted(image, 9, -6), first)
        self.assertIsNotNone(followed, "tracks a 9 px move")
        self.assertEqual(followed.d.data if followed.d else None, truth.d)
        for result, expected in zip(followed.fields, truth.fields):
            self.assertEqual(result.data if result else None, expected)
        self.assertIsNone(followed.inferred_corner)
        # more than a finder diameter: tracking refuses, a full decode is needed
        jumped = shifted(image, 95, 0)
        self.assertIsNone(track_frame(jumped, first))
        decode_frame(jumped)

    def test_tracking_starts_the_distortion_search_from_the_previous_pose(self) -> None:
        image = setup(4)
        first = decode_frame(image)
        canonical = [(float(x), float(y)) for x, y in FINDER_CENTERS]
        points = [first.pose.apply(x, y) for x, y in canonical]
        bent = Pose.fit(canonical, points, -0.04, image.width, image.height)
        self.assertIsNotNone(bent)
        tried = []
        read_field_at = _read_field_at

        def spy(image_, dst, distortion, inferred):
            tried.append(distortion)
            return read_field_at(image_, dst, distortion, inferred)

        with mock.patch("iroha_petal.decode._read_field_at", side_effect=spy):
            followed = track_frame(image, dataclasses.replace(first, pose=bent))
            self.assertIsNotNone(followed)
            self.assertEqual(tried[0], -0.04)
            for distortion, step in zip(tried[1:], _DISTORTION_STEPS):
                self.assertEqual(distortion, -0.04 + step)
            # a full decode starts from a plain pose
            tried.clear()
            decode_frame(image)
            self.assertEqual(tried[0], 0.0)
        self.assertEqual((followed.rotation, followed.mirrored), (first.rotation, first.mirrored))

    def test_broken_poses_are_refused_without_crashing(self) -> None:
        image = setup(4)
        previous = decode_frame(image)
        self.assertIsNotNone(track_frame(image, previous))
        # a good pose that names a corner that does not exist
        for corner in (4, 255, -1):
            named = dataclasses.replace(previous, inferred_corner=corner)
            self.assertIsNone(track_frame(image, named), corner)
        nan, inf = float("nan"), float("inf")
        non_finite = [
            Homography((nan,) * 9),
            Homography((inf, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)),
        ]
        for broken in non_finite:
            self.assertIsNone(decode_frame_at(image, Pose.plain(broken)))
        # the last one makes every finder far larger than the image
        huge = Homography((50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 1.0))
        for broken in non_finite + [huge]:
            for inferred in (None, 2):
                stale = dataclasses.replace(
                    previous, pose=Pose.plain(broken), inferred_corner=inferred
                )
                self.assertIsNone(track_frame(image, stale), (broken, inferred))
        # an unusable image is refused before any work
        self.assertIsNone(track_frame(Luma(40, 40), previous))

    def test_tracking_survives_a_blossom_that_disappears(self) -> None:
        first = decode_frame(setup(4))
        # the same code, slightly moved, now with the bottom-left blossom covered
        moved = shifted(hidden_blossom(4, 3), -5, 4)
        followed = track_frame(moved, first)
        self.assertIsNotNone(followed, "tracks with three blossoms")
        self.assertEqual(followed.inferred_corner, 3)
        self.assertEqual(followed.d.data if followed.d else None, lane_data(4).d)
        self.assertEqual((followed.rotation, followed.mirrored), (first.rotation, first.mirrored))

    def test_a_blossom_that_reappears_is_seen_again(self) -> None:
        covered = hidden_blossom(4, 3)
        first = decode_frame(covered)
        self.assertEqual(first.inferred_corner, 3)
        # the thumb moves away and the hand moves a little
        followed = track_frame(shifted(setup(4), 4, -3), first)
        self.assertIsNotNone(followed, "tracks")
        self.assertIsNone(followed.inferred_corner)
        self.assertEqual(followed.d.data if followed.d else None, lane_data(4).d)
        # still covered: still inferred
        followed = track_frame(shifted(covered, 4, -3), first)
        self.assertIsNotNone(followed, "tracks")
        self.assertEqual(followed.inferred_corner, 3)

    def test_two_lost_blossoms_end_tracking(self) -> None:
        first = decode_frame(setup(4))
        image = hidden_blossom(4, 3)
        n = image.width
        data = bytearray(image.data)
        # also cover the top-right blossom
        for y in range(0, n // 4):
            data[y * n + n * 3 // 4 : y * n + n] = bytes(n - n * 3 // 4)
        self.assertIsNone(track_frame(Luma(n, n, bytes(data)), first))


if __name__ == "__main__":
    unittest.main()
