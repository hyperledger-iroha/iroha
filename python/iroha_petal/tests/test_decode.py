# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Frame decoding on rendered frames, transformed frames and garbage images."""

from __future__ import annotations

import unittest
from unittest import mock

from petal_test_support import (
    decode_test_payload,
    encoder,
    rendered,
    rotate90,
    rotate180,
    rotate270,
    tile_lanes,
)

from iroha_petal import (
    D_DATA,
    D_WORD,
    FINDER_CENTERS,
    K_DATA,
    P_DATA,
    P_WORD,
    Beacon,
    DecodeError,
    DecodeErrorKind,
    DecodeOptions,
    FrameCells,
    Homography,
    Lane,
    Luma,
    RenderOptions,
    RsError,
    RsErrorKind,
    StreamAssembler,
    Xorshift32,
    decode_frame,
    decode_frame_at,
    encode_lane,
    observed_cells,
    render_frame,
    tile_center,
    tile_match_error,
)
from iroha_petal._numeric import round_half_away
from iroha_petal.decode import (
    _build_patterns,
    _classify,
    _decode_with_erasures,
    _patch_levels,
    _projector,
    _read_tile_lanes,
    _read_tiles,
    _read_tiles_normalised,
    _reference_levels,
    _rescale,
    _rescaled_patterns,
    _sample_patches,
)

KIND = 2

#: The words the reference accepts in its scan of 40 000 random words per lane (trial number:
#: erasures, rewritten bytes, data): three of lane D, none of lane P.
SCAN_ACCEPTED_D = {
    9827: (5, 8, "c0300c3dfecddbccf0a64620e94c3fd1828690"),
    12427: (5, 8, "1e2795eb69f8b6bf6dd8da502f87aec4998df8"),
    33444: (5, 8, "a7f57ed4c0da100977229890cfb77829a7a9bc"),
}


def setup(frame: int) -> Luma:
    """Frame ``frame`` of the reference decoder-test stream, 768 px, 2x2 supersampled."""
    return rendered(decode_test_payload(), KIND, frame, 768, 2).to_luma()


def lane_data(frame: int):
    return encoder(decode_test_payload(), KIND).lane_data(frame)


def render_homography() -> Homography:
    """The exact canvas-to-pixel homography of the 768-pixel test renders."""
    canonical = [(float(x), float(y)) for x, y in FINDER_CENTERS]
    scale = 768.0 / 1024.0
    pixels = [(x * scale, y * scale) for x, y in canonical]
    homography = Homography.from_points(canonical, pixels)
    assert homography is not None
    return homography


def clean_patches(frame: int):
    """A clean 768-pixel render with the exact canvas-to-pixel homography and its raw patches."""
    luma = setup(frame)
    h = render_homography()
    return luma, h, _sample_patches(_projector(luma, h))


def distort(patches):
    """Gives every tile its own gain and offset, as under glare, shadows and saturation."""
    distorted = []
    for tile, raw in enumerate(patches):
        gain = 0.35 + 0.65 * ((tile * 37 % 101) / 100.0)
        offset = 5.0 + float(tile * 53 % 61)
        distorted.append([gain * value + offset for value in raw])
    return distorted


class DecodeTest(unittest.TestCase):
    def assert_error(self, image: Luma, kind: DecodeErrorKind, options=None) -> None:
        with self.assertRaises(DecodeError) as caught:
            decode_frame(image, options)
        self.assertIs(caught.exception.kind, kind)

    def test_clean_render_decodes_every_lane(self) -> None:
        decoded = decode_frame(setup(5))
        p, k, d = lane_data(5)
        self.assertEqual(decoded.p.data, p)
        self.assertEqual(decoded.k.data, k)
        self.assertEqual(decoded.d.data, d)
        self.assertEqual((decoded.rotation, decoded.mirrored), (0, False))
        self.assertEqual((decoded.lanes, decoded.lanes_ok), ("PKD", 3))
        self.assertEqual((decoded.p.erasures, decoded.p.corrected), (0, 0))

    def test_rotated_and_mirrored_renders_decode_with_the_right_orientation(self) -> None:
        image = setup(7)
        p, _, d = lane_data(7)
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
            self.assertEqual(decoded.d.data, d, f"{name} lane D")
            self.assertEqual(decoded.p.data, p, f"{name} lane P")
        with self.assertRaises(DecodeError) as caught:
            decode_frame(image.mirrored(), DecodeOptions(try_mirrored=False))
        self.assertIs(caught.exception.kind, DecodeErrorKind.NO_ORIENTATION)

    def test_lane_k_alone_is_enough_to_accept_an_orientation(self) -> None:
        stream = encoder(decode_test_payload(), KIND)
        _, k_word, _ = stream.words(5)
        # lanes P and D are junk (not codewords); only the katakana lane carries data
        rng = Xorshift32(7)
        junk_p = bytes(rng.next_byte() for _ in range(P_WORD))
        junk_d = bytes(rng.next_byte() for _ in range(D_WORD))
        cells = FrameCells.from_words(junk_p, k_word, junk_d)
        image = render_frame(cells, RenderOptions(size=768, supersample=2)).to_luma()
        decoded = decode_frame(image)
        self.assertEqual(decoded.lanes, "K")
        self.assertEqual(decoded.k.data, stream.lane_data(5)[1])
        self.assertEqual((decoded.rotation, decoded.mirrored), (0, False))

    def test_corrected_counts_rewritten_bytes_not_just_erasures(self) -> None:
        data = bytes(range(P_DATA))
        word = bytearray(encode_lane(Lane.P, data))
        for position in (2, 11, 30):
            word[position] ^= 0x5A
        result = _decode_with_erasures(Lane.P, bytes(word), [1.0] * len(word))
        self.assertIsNotNone(result, "three errors fit")
        self.assertEqual(result.data, data)
        self.assertEqual((result.erasures, result.corrected), (0, 3))
        # with the damaged bytes flagged as least confident, they become erasures
        flagged = [1.0] * len(word)
        for position in (2, 11, 30):
            flagged[position] = 0.0
        result = _decode_with_erasures(Lane.P, bytes(word), flagged)
        self.assertEqual(result.data, data)
        self.assertGreaterEqual(result.corrected, 3)
        # too much damage for the zero-erasure attempt: the least confident bytes
        # become erasures and still count as rewritten
        for position in (4, 5, 6, 7):
            word[position] ^= 0x33
            flagged[position] = 0.0
        result = _decode_with_erasures(Lane.P, bytes(word), flagged)
        self.assertEqual(result.data, data)
        self.assertGreater(result.erasures, 0)
        self.assertGreaterEqual(result.corrected, 7)

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
        # a buffer that does not match the stated size must not reach the sampler
        self.assertIsNone(decode_frame_at(broken, Homography.IDENTITY))
        decoded = decode_frame(setup(5))
        self.assertIsNone(observed_cells(broken, decoded))
        self.assertIsNone(tile_match_error(broken, decoded))
        huge = Luma(5_000, 3_000)
        self.assert_error(huge, unsupported)
        self.assertIsNone(decode_frame_at(Luma(8, 8), Homography.IDENTITY))

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
        # exercises the locator, quad selection and homography on degenerate layouts.
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

    def test_a_valid_code_with_a_missing_finder_is_not_misread(self) -> None:
        image = setup(2)
        n = image.width
        data = bytearray(image.data)
        # erase the bottom-right blossom
        for y in range(n * 3 // 4, n):
            data[y * n + n * 3 // 4 : y * n + n] = bytes(n - n * 3 // 4)
        with self.assertRaises(DecodeError):
            decode_frame(Luma(n, n, bytes(data)))

    def test_known_pose_decoding_reads_every_lane(self) -> None:
        image = setup(4)
        scale = image.width / 1024.0
        pose = Homography((scale, 0.0, 0.0, 0.0, scale, 0.0, 0.0, 0.0, 1.0))
        decoded = decode_frame_at(image, pose)
        self.assertIsNotNone(decoded)
        self.assertEqual(decoded.lanes, "PKD")
        p, k, d = lane_data(4)
        self.assertEqual((decoded.p.data, decoded.k.data, decoded.d.data), (p, k, d))
        beacon = decoded.beacon()
        self.assertIsInstance(beacon, Beacon)
        self.assertEqual(beacon.meta, encoder(decode_test_payload(), KIND).meta)
        self.assertIsNone(decode_frame_at(Luma(320, 240), pose), "weak reference levels")

    def test_frames_feed_an_assembler(self) -> None:
        stream = encoder(decode_test_payload(), KIND)
        assembler = StreamAssembler()
        for frame in range(stream.systematic_frames()):
            decoded = decode_frame(setup(frame))
            packets = decoded.atom_packets()
            self.assertEqual(sum(len(p.atoms) for p in packets), 6 if frame % 4 == 0 else 7)
            decoded.feed(assembler)
        completed = assembler.take_completed()
        self.assertIsNotNone(completed)
        self.assertEqual(completed.payload, decode_test_payload())

    def test_max_side_downscales_large_inputs_and_keeps_pixel_coordinates(self) -> None:
        image = rendered(decode_test_payload(), KIND, 5, 1024, 1).to_luma()
        decoded = decode_frame(image, max_side=512)
        self.assertEqual(decoded.lanes, "PKD")
        for canvas in ((512.0, 512.0), (72.0, 72.0), (952.0, 952.0)):
            x, y = decoded.homography.apply(*canvas)
            self.assertLess(abs(x - canvas[0]), 2.0)
            self.assertLess(abs(y - canvas[1]), 2.0)
        with self.assertRaises(ValueError):
            decode_frame(image, max_side=10)

    def test_diagnostics_report_the_cells_seen(self) -> None:
        image = setup(5)
        decoded = decode_frame(image)
        cells = observed_cells(image, decoded)
        self.assertEqual(cells, encoder(decode_test_payload(), KIND).cells(5))
        error = tile_match_error(image, decoded)
        self.assertGreaterEqual(error, 0.0)
        self.assertLess(error, 0.5)


class ErasureScheduleTest(unittest.TestCase):
    """How many erasures each lane's Reed-Solomon decoding is allowed to try."""

    def test_random_words_are_almost_never_accepted(self) -> None:
        # Reed-Solomon with erasures can accept a word that is not a transmission. Lane D has only
        # 11 parity bytes, so its schedule stops at five erasures (at seven it let through about
        # one random word in 250) and lane P stops at six. The reference scans 40 000 xorshift32
        # words per lane (lane D, then lane P from the same generator), each followed by one
        # byte-valued confidence per position, and accepts exactly SCAN_ACCEPTED_D and no word of
        # lane P. Both full scans take about 40 s here and the port reproduces them exactly; this
        # runs the first 12 500 words of lane D and the first 2 000 of lane P.
        scan = 40_000
        rng = Xorshift32(0x5EED)
        for lane, run in ((Lane.D, 12_500), (Lane.P, 2_000)):
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
                # leave the generator where the reference's scan of lane P starts
                for _ in range((scan - run) * 2 * length):
                    rng.next_u32()
            else:
                expected = {}
            self.assertEqual(accepted, expected, f"lane {lane.name} of {run} random words")

    def test_only_lane_k_uses_two_thirds_of_its_parity_as_erasures(self) -> None:
        # damaged bytes: `flagged` of them marked least confident, two more hidden. With the extra
        # erasure step of the old schedule the decoder would repair them (2*2 + flagged parity
        # bytes); the capped schedule must refuse instead of risking a wrong codeword.
        for lane, flagged in ((Lane.D, 7), (Lane.P, 8)):
            data = bytes(range(lane.data_len))
            word = encode_lane(lane, data)
            damaged = bytearray(word)
            confidence = [1.0] * len(word)
            for position in range(flagged):
                damaged[position] ^= 0xA5
                confidence[position] = 0.0
            damaged[20] ^= 0x3C
            damaged[21] ^= 0x3C
            result = _decode_with_erasures(lane, bytes(damaged), confidence)
            self.assertIsNone(result, f"lane {lane.name}")
            # half the parity flagged plus one hidden error stays comfortably repairable
            damaged = bytearray(word)
            confidence = [1.0] * len(damaged)
            for position in range(lane.parity_len // 2):
                damaged[position] ^= 0xA5
                confidence[position] = 0.0
            damaged[20] ^= 0x3C
            result = _decode_with_erasures(lane, bytes(damaged), confidence)
            self.assertIsNotNone(result, f"lane {lane.name} repairable")
            self.assertEqual(result.data, data, f"lane {lane.name}")
            self.assertLessEqual(result.erasures, lane.parity_len // 2, f"lane {lane.name}")
        # lane K keeps the two-thirds step: 30 flagged bytes plus 7 hidden errors need it
        # (2*7 + 30 = 44 of 45 parity bytes)
        data = bytes(range(K_DATA))
        damaged = bytearray(encode_lane(Lane.K, data))
        confidence = [1.0] * len(damaged)
        for position in range(30):
            damaged[position] ^= 0xA5
            confidence[position] = 0.0
        for position in range(60, 67):
            damaged[position] ^= 0x3C
        result = _decode_with_erasures(Lane.K, bytes(damaged), confidence)
        self.assertIsNotNone(result, "30 erasures")
        self.assertEqual((result.data, result.erasures), (data, 30))

    def test_each_lane_tries_its_own_erasure_counts(self) -> None:
        # integer eighths, quarters, thirds and halves of the parity; two thirds only for K
        expected = {
            Lane.D: [0, 1, 2, 3, 5],
            Lane.P: [0, 1, 3, 4, 6],
            Lane.K: [0, 5, 11, 15, 22, 30],
        }
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
        # Every tile the normalised read erases has confidence exactly 0, so ties are the rule, and
        # the ranking must be stable or ports disagree about which bytes are erased. Three damaged
        # bytes at the front plus four hidden ones fit lane D only if exactly the first three
        # positions are erased (3 erasures + 4 errors = all 11 parity bytes): a step that erased
        # the last positions instead would see seven errors.
        data = bytes(range(D_DATA))
        word = bytearray(encode_lane(Lane.D, data))
        for position in (0, 1, 2, 20, 21, 22, 23):
            word[position] ^= 0x5A
        confidence = [1.0] * len(word)
        result = _decode_with_erasures(Lane.D, bytes(word), confidence)
        self.assertIsNotNone(result, "ties in order")
        self.assertEqual((result.data, result.erasures, result.corrected), (data, 3, 7))


class TileReadTest(unittest.TestCase):
    """The level read and the normalised read of the tile lanes."""

    def test_sampled_patches_are_the_mean_of_four_bilinear_samples_per_cell(self) -> None:
        # an arbitrary but deterministic texture, so that no two samples coincide
        side = 256
        image = Luma(
            side,
            side,
            bytes((x * 7 + y * 13 + x * y % 11) % 256 for y in range(side) for x in range(side)),
        )
        scale = side / 1024.0
        pose = Homography((scale, 0.01, 1.5, -0.01, scale, 2.5, 1e-5, 2e-5, 1.0))
        patches = _sample_patches(_projector(image, pose))
        self.assertEqual((len(patches), {len(p) for p in patches}), (256, {64}))
        cell = 23.0 / 8.0
        for tile in (0, 37, 128, 255):
            cx, cy = tile_center(tile)
            for v in range(8):
                for u in range(8):
                    gx = cx - 23.0 / 2.0 + (u + 0.5) * cell
                    gy = cy - 23.0 / 2.0 + (v + 0.5) * cell
                    total = 0.0
                    for ox, oy in ((-0.25, -0.25), (0.25, -0.25), (-0.25, 0.25), (0.25, 0.25)):
                        px, py = pose.apply(gx + ox * cell, gy + oy * cell)
                        total += image.sample(px, py)
                    self.assertEqual(patches[tile][v * 8 + u], total / 4.0, (tile, v, u))

    def test_level_and_normalised_reads_agree_on_a_clean_render(self) -> None:
        luma, h, patches = clean_patches(5)
        p_data, k_data, _ = lane_data(5)
        sigmas = DecodeOptions().template_sigmas
        reference = _reference_levels(_projector(luma, h))
        self.assertIsNotNone(reference)
        for name, reads in (
            ("level", _read_tiles(patches, reference, sigmas)),
            ("normalised", _read_tiles_normalised(patches, sigmas)),
        ):
            p, k = tile_lanes(reads)
            self.assertIsNotNone(p, f"{name}: lane P")
            self.assertIsNotNone(k, f"{name}: lane K")
            self.assertEqual((p.data, p.corrected), (p_data, 0), name)
            self.assertEqual((k.data, k.corrected), (k_data, 0), name)

    def test_normalised_read_cancels_gain_and_offset_per_tile(self) -> None:
        luma, h, patches = clean_patches(5)
        p_data, k_data, _ = lane_data(5)
        sigmas = DecodeOptions().template_sigmas
        distorted = distort(patches)
        reference = _reference_levels(_projector(luma, h))
        self.assertIsNotNone(reference)
        p, _ = tile_lanes(_read_tiles(distorted, reference, sigmas))
        self.assertIsNone(
            p, "the level read must not survive this distortion, or the test proves nothing"
        )
        p, k = tile_lanes(_read_tiles_normalised(distorted, sigmas))
        self.assertIsNotNone(p, "lane P")
        self.assertIsNotNone(k, "lane K")
        self.assertEqual((p.data, k.data), (p_data, k_data))

    def test_normalised_read_erases_tiles_that_lost_their_contrast(self) -> None:
        _, _, patches = clean_patches(5)
        patches[5] = [100.0] * 64
        patches[9] = [30.0] * 64
        reads = _read_tiles_normalised(patches, DecodeOptions().template_sigmas)
        for tile in (5, 9):
            self.assertLess(abs(reads[tile].polarity_margin), 1e-12, f"tile {tile}")
            self.assertLess(abs(reads[tile].glyph_margin), 1e-12, f"tile {tile}")
        self.assertGreater(reads[6].polarity_margin, 0.0)
        self.assertGreater(reads[6].glyph_margin, 0.0)

    def test_normalised_read_does_not_amplify_contrast_below_one_level(self) -> None:
        _, _, patches = clean_patches(5)
        sigmas = DecodeOptions().template_sigmas
        sharp = _read_tiles_normalised(patches, sigmas)[6]
        # the same tile squeezed into a total contrast of 0.3 luma levels: the span floor of
        # one level stops it from being stretched back into a perfect match
        low, high = _patch_levels(patches[6])
        patches[6] = [100.0 + 0.3 * (value - low) / (high - low) for value in patches[6]]
        faint = _read_tiles_normalised(patches, sigmas)[6]
        self.assertLess(sharp.error, 0.5)
        self.assertGreater(faint.error, 5.0)
        self.assertEqual((faint.polarity_margin, faint.glyph_margin), (0.0, 0.0))

    def test_patch_levels_ignore_the_extreme_cells(self) -> None:
        values = [10.0] * 64
        for i in range(32):
            values[i] = 200.0 + float(i % 3)
        values[0] = 255.0  # one hot cell
        values[63] = 0.0  # one dead cell
        low, high = _patch_levels(values)
        self.assertLess(abs(low - 10.0), 1e-12)
        self.assertTrue(200.0 <= high <= 202.0)
        scaled = _rescale(values, 1.0)
        self.assertTrue(all(-0.25 <= v <= 1.25 for v in scaled))
        # a flat patch stays flat instead of dividing by nothing
        self.assertTrue(all(abs(v) < 1e-12 for v in _rescale([7.0] * 64, 1.0)))

    def test_patch_levels_pick_the_sixth_and_fifty_seventh_sorted_cell(self) -> None:
        self.assertEqual(_patch_levels([float(i) for i in range(64)]), (6.0, 57.0))
        # the order of the cells does not matter, only their ranks
        shuffled = [float(i * 17 % 64) for i in range(64)]
        self.assertEqual(sorted(shuffled), [float(i) for i in range(64)])
        self.assertEqual(_patch_levels(shuffled), (6.0, 57.0))

    def test_patch_levels_rank_nan_cells_last_like_total_cmp(self) -> None:
        nan = float("nan")
        # a plain sort would leave leading NaNs in front and shift both ranks
        values = [nan] * 4 + [float(i) for i in range(60)]
        self.assertEqual(_patch_levels(values), (6.0, 57.0))

    def test_weak_tiles_are_judged_against_the_median_contrast(self) -> None:
        _, _, patches = clean_patches(5)
        # Sorted by contrast, the median (element 128) is the single tile of span 40; a
        # quarter of it is 10. Tiles of span 8 are weak, those of span 11 and above are not.
        spans = [8.0] * 125 + [11.0] * 3 + [40.0] + [400.0] * 127
        self.assertEqual(len(spans), 256)
        shaped = []
        for raw, span in zip(patches, spans):
            low, high = _patch_levels(raw)
            shaped.append([100.0 + span * (value - low) / (high - low) for value in raw])
        reads = _read_tiles_normalised(shaped, DecodeOptions().template_sigmas)
        for tile, span in enumerate(spans):
            weak = (reads[tile].polarity_margin, reads[tile].glyph_margin) == (0.0, 0.0)
            self.assertEqual(weak, span == 8.0, f"tile {tile} with span {span}")

    def test_rescaled_templates_span_their_own_contrast(self) -> None:
        for sigma in DecodeOptions().template_sigmas:
            plain = _build_patterns(sigma)
            rescaled = _rescaled_patterns(sigma)
            self.assertEqual(len(rescaled), 32)
            for before, after in zip(plain, rescaled):
                self.assertEqual(_patch_levels(after), (0.0, 1.0), f"sigma {sigma}")
                self.assertTrue(all(-0.25 <= v <= 1.25 for v in after), f"sigma {sigma}")
                self.assertNotEqual(before, after, f"sigma {sigma}")

    def test_classify_compares_against_rescaled_templates_only_when_asked(self) -> None:
        sigmas = (0.5,)
        not_erased = [False] * 256
        for rescale_templates in (False, True):
            templates = (_rescaled_patterns if rescale_templates else _build_patterns)(0.5)
            # every tile shows one of the 32 templates exactly: that template must win, with no error
            patches = [list(templates[tile % 32]) for tile in range(256)]
            reads = _classify(patches, sigmas, rescale_templates, not_erased)
            self.assertEqual(len(reads), 256)
            for tile, read in enumerate(reads):
                expected = (tile % 32 >= 16, tile % 16, 0.0)
                self.assertEqual(
                    (read.light, read.glyph, read.error),
                    expected,
                    f"rescale_templates={rescale_templates} tile {tile}",
                )
            # the other template set does not explain these patches exactly
            other = _classify(patches, sigmas, not rescale_templates, not_erased)
            self.assertTrue(any(read.error > 0.0 for read in other))

    def test_shadowed_part_of_a_render_decodes_through_the_normalised_read(self) -> None:
        p_data, k_data, _ = lane_data(5)
        image = setup(5)
        width = image.width
        dim = bytes(int(round_half_away(float(v) * 0.3)) for v in range(256))
        rows = []
        for row in range(image.height):
            line = bytes(image.data[row * width : (row + 1) * width])
            first, last = width * 7 // 20, width * 3 // 5
            rows.append(line[:first] + line[first:last].translate(dim) + line[last:])
        luma = Luma(width, image.height, b"".join(rows))
        # the finder levels cannot describe a step in the light: the level read loses lane K
        h = render_homography()
        project = _projector(luma, h)
        reference = _reference_levels(project)
        self.assertIsNotNone(reference)
        reads = _read_tiles(_sample_patches(project), reference, DecodeOptions().template_sigmas)
        _, k = tile_lanes(reads)
        self.assertIsNone(k)
        decoded = decode_frame(luma)
        self.assertIsNotNone(decoded.p)
        self.assertIsNotNone(decoded.k)
        self.assertEqual(decoded.p.data, p_data)
        self.assertEqual(decoded.k.data, k_data)

    def test_the_normalised_read_runs_only_for_missing_lanes(self) -> None:
        luma, h, patches = clean_patches(5)
        p_data, k_data, _ = lane_data(5)
        sigmas = DecodeOptions().template_sigmas
        reference = _reference_levels(_projector(luma, h))
        self.assertIsNotNone(reference)
        with mock.patch(
            "iroha_petal.decode._read_tiles_normalised", wraps=_read_tiles_normalised
        ) as normalised:
            p, k = _read_tile_lanes(patches, reference, sigmas)
            self.assertEqual((p.data, k.data), (p_data, k_data))
            normalised.assert_not_called()
            # per-tile gain and offset break the level read (see the test above)
            p, k = _read_tile_lanes(distort(patches), reference, sigmas)
            self.assertEqual((p.data, k.data), (p_data, k_data))
            normalised.assert_called_once()

    def test_degenerate_patches_and_poses_never_crash_the_tile_reads(self) -> None:
        sigmas = DecodeOptions().template_sigmas
        nan = float("nan")
        # NaN patches match nothing: no reads at all, as in the reference
        self.assertEqual(_read_tiles_normalised([[nan] * 64 for _ in range(256)], sigmas), [])
        # a flat frame carries no contrast: nothing decodes
        flat = _read_tiles_normalised([[7.0] * 64 for _ in range(256)], sigmas)
        self.assertEqual(tile_lanes(flat), (None, None))
        # poses that map everything to NaN reach the tile reads and must not raise
        image = setup(2)
        for pose in (Homography((nan,) * 9), Homography((0.0,) * 9)):
            decoded = decode_frame_at(image, pose)
            self.assertTrue(decoded is None or decoded.lanes_ok == 0)


if __name__ == "__main__":
    unittest.main()
