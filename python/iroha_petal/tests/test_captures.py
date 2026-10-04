# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Golden camera captures in ``fixtures/petal/petal_captures_v1.json``.

Every conforming decoder must read the lanes named in ``must_decode``, must
never report wrong data for any lane, must report the recorded inferred corner,
must follow each tracking pair from its first frame into its second, and must
reject the negatives. This port follows the reference operation by operation,
so it is also held to the exact lanes the reference read (``reference_decoded``
and ``reference_tracked``).
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
    track_frame,
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
    "hidden-corner-540p",
    "cut-corner-720p",
]

#: The tracking pairs of the fixture, in order.
TRACK_NAMES = ["steady-hand-540p", "thumb-arrives-540p"]

#: Tile lanes that the level read (judging patches against the finder levels) cannot read on
#: the lighting captures; only the normalised read gets them.
LEVEL_READ_MISSES = {"overexposed-540p": "PK", "veiled-720p": "K", "shadow-band-540p": "PK"}


class CaptureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = captures_fixture()

    def test_the_fixture_carries_eleven_captures_two_tracks_and_two_negatives(self) -> None:
        self.assertEqual([c["name"] for c in self.doc["captures"]], CAPTURE_NAMES)
        self.assertEqual([t["name"] for t in self.doc["tracks"]], TRACK_NAMES)
        self.assertEqual(len(self.doc["negatives"]), 2)
        inferred = {c["name"]: c["inferred_corner"] for c in self.doc["captures"]}
        self.assertEqual(
            {name: corner for name, corner in inferred.items() if corner is not None},
            {"hidden-corner-540p": 3, "cut-corner-720p": 2},
        )

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
            self.assertEqual(
                decoded.inferred_corner, capture["inferred_corner"], f"{name}: inferred corner"
            )
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

    def test_golden_tracks_follow_the_pose_into_the_next_frame(self) -> None:
        timings = []
        for pair in self.doc["tracks"]:
            name = pair["name"]
            first = luma_of(pair, "from_luma_zlib_base64")
            try:
                previous = decode_frame(first)
            except DecodeError as error:  # pragma: no cover - failure path
                self.fail(f"{name}: first frame: {error}")
            started = time.perf_counter()
            followed = track_frame(luma_of(pair, "to_luma_zlib_base64"), previous)
            elapsed = time.perf_counter() - started
            self.assertIsNotNone(followed, f"{name}: tracking lost the code")
            timings.append((name, followed.lanes, elapsed))
            must = pair["must_track"]
            for letter, result in (("P", followed.p), ("K", followed.k), ("D", followed.d)):
                if result is None:
                    self.assertNotIn(letter, must, f"{name}: lane {letter} lost")
                else:
                    expected = bytes.fromhex(pair[f"{letter.lower()}_data"])
                    self.assertEqual(result.data, expected, f"{name}: lane {letter} data")
            self.assertEqual(followed.lanes, pair["reference_tracked"], f"{name}: lanes")
            self.assertEqual(
                followed.inferred_corner, pair["inferred_corner"], f"{name}: inferred corner"
            )
            # the orientation is kept from the first frame
            self.assertEqual(
                (followed.rotation, followed.mirrored), (previous.rotation, previous.mirrored)
            )
        for name, lanes, elapsed in timings:
            sys.stderr.write(f"\n  {name:<22} tracked lanes {lanes:<3} {elapsed:.2f} s")
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
        for pair in self.doc["tracks"]:
            p, k, d = encoder.lane_data(pair["to_frame"])
            self.assertEqual(
                (p.hex(), k.hex(), d.hex()),
                (pair["p_data"], pair["k_data"], pair["d_data"]),
                pair["name"],
            )

    def test_renderer_reproduces_the_clean_capture_pixel_for_pixel(self) -> None:
        capture = next(c for c in self.doc["captures"] if c["name"] == "clean-512")
        encoder = StreamEncoder(bytes.fromhex(self.doc["payload_hex"]), self.doc["payload_kind"])
        rgb = render_frame(encoder.cells(capture["frame"]), RenderOptions(size=512, supersample=3))
        self.assertEqual(rgb.to_luma().data, luma_of(capture).data)


if __name__ == "__main__":
    unittest.main()
