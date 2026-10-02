# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Golden camera captures in ``fixtures/petal/petal_captures_v1.json``.

Every conforming decoder must read the lanes named in ``must_decode``, must
never report wrong data for any lane, and must reject the negatives. This port
follows the reference operation by operation, so it is also held to the exact
lanes the reference read (``reference_decoded``).
"""

from __future__ import annotations

import sys
import time
import unittest

from petal_test_support import captures_fixture, luma_of, payload, stream_fixture, tile_lanes

from iroha_petal import (
    DecodeError,
    DecodeOptions,
    Lane,
    RenderOptions,
    StreamAssembler,
    StreamEncoder,
    decode_frame,
    decode_lane,
    encode_lane,
    render_frame,
)
from iroha_petal.decode import _projector, _read_tiles, _reference_levels, _sample_patches

#: Generous per-capture budget for plain CPython (the port takes about a second).
TIME_BUDGET_S = 30.0

#: The captures of the fixture, in order.
CAPTURE_NAMES = [
    "clean-512",
    "modern-720p-rotated",
    "legacy-540p-tilted",
    "soft-480p-blur1.9",
    "small-480p",
    "selfie-mirrored-540p",
    "overexposed-540p",
    "veiled-720p",
    "shadow-band-540p",
]

#: Tile lanes that the level read (judging patches against the finder levels) cannot read on
#: the lighting captures; only the normalised read gets them.
LEVEL_READ_MISSES = {"overexposed-540p": "PK", "veiled-720p": "K", "shadow-band-540p": "PK"}


class CaptureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = captures_fixture()

    def test_the_fixture_carries_the_nine_captures_and_two_negatives(self) -> None:
        self.assertEqual([c["name"] for c in self.doc["captures"]], CAPTURE_NAMES)
        self.assertEqual(len(self.doc["negatives"]), 2)

    def test_golden_captures_decode_as_recorded(self) -> None:
        assembler = StreamAssembler()
        timings = []
        for capture in self.doc["captures"]:
            name = capture["name"]
            image = luma_of(capture)
            started = time.perf_counter()
            try:
                decoded = decode_frame(image)
            except DecodeError as error:  # pragma: no cover - failure path
                self.fail(f"{name}: {error}")
            elapsed = time.perf_counter() - started
            timings.append((name, image.width, image.height, decoded.lanes, elapsed))
            self.assertLess(elapsed, TIME_BUDGET_S, f"{name}: decode took {elapsed:.1f} s")
            self.assertEqual(decoded.mirrored, capture["mirrored"], f"{name}: mirror flag")
            must = capture["must_decode"]
            for letter, result in (("P", decoded.p), ("K", decoded.k), ("D", decoded.d)):
                expected = bytes.fromhex(capture[f"{letter.lower()}_data"])
                if result is None:
                    self.assertNotIn(
                        letter, must, f"{name}: required lane {letter} was not decoded"
                    )
                else:
                    self.assertEqual(result.data, expected, f"{name}: lane {letter} data")
            self.assertEqual(decoded.lanes, capture["reference_decoded"], f"{name}: lanes")
            decoded.feed(assembler)
        # captures of different frames of the same stream accumulate in one assembler
        self.assertGreater(assembler.progress().atoms_received, 10)
        for name, width, height, lanes, elapsed in timings:
            sys.stderr.write(f"\n  {name:<22} {width}x{height} lanes {lanes:<3} {elapsed:.2f} s")
        sys.stderr.write("\n")

    def test_lighting_captures_need_the_normalised_read(self) -> None:
        by_name = {c["name"]: c for c in self.doc["captures"]}
        sigmas = DecodeOptions().template_sigmas
        for name, missed in LEVEL_READ_MISSES.items():
            image = luma_of(by_name[name])
            decoded = decode_frame(image)
            project = _projector(image, decoded.homography)
            reference = _reference_levels(project)
            self.assertIsNotNone(reference, name)
            p, k = tile_lanes(_read_tiles(_sample_patches(project), reference, sigmas))
            level = "".join(letter for letter, lane in (("P", p), ("K", k)) if lane is not None)
            for letter in "PK":
                if letter in missed:
                    self.assertNotIn(letter, level, f"{name}: the level read already gets {letter}")
                    self.assertIn(letter, decoded.lanes, f"{name}: lane {letter} not recovered")
                else:
                    self.assertIn(letter, level, f"{name}: the level read lost {letter}")

    def test_negative_captures_are_rejected(self) -> None:
        for negative in self.doc["negatives"]:
            with self.assertRaises(DecodeError, msg=negative["name"]):
                decode_frame(luma_of(negative))

    def test_recorded_lane_data_is_a_valid_codeword_of_its_stream(self) -> None:
        for capture in self.doc["captures"]:
            data = bytes.fromhex(capture["p_data"])
            word = encode_lane(Lane.P, data)
            self.assertEqual(decode_lane(Lane.P, word), data)

    def test_captures_show_the_one_pass_stream(self) -> None:
        stream = next(s for s in stream_fixture()["streams"] if s["name"] == "one-pass")
        data = bytes.fromhex(self.doc["payload_hex"])
        self.assertEqual(data, bytes.fromhex(stream["payload_hex"]))
        self.assertEqual(data, payload(700, 2))
        encoder = StreamEncoder(data, self.doc["payload_kind"])
        for capture in self.doc["captures"]:
            p, k, d = encoder.lane_data(capture["frame"])
            self.assertEqual(
                (p.hex(), k.hex(), d.hex()),
                (capture["p_data"], capture["k_data"], capture["d_data"]),
                capture["name"],
            )

    def test_renderer_reproduces_the_clean_capture_pixel_for_pixel(self) -> None:
        capture = next(c for c in self.doc["captures"] if c["name"] == "clean-512")
        encoder = StreamEncoder(bytes.fromhex(self.doc["payload_hex"]), self.doc["payload_kind"])
        rgb = render_frame(encoder.cells(capture["frame"]), RenderOptions(size=512, supersample=3))
        self.assertEqual(rgb.to_luma().data, luma_of(capture).data)


if __name__ == "__main__":
    unittest.main()
