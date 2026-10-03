# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""The scan session: decoding, tracking, reassembly, timeouts and statistics."""

from __future__ import annotations

import math
import unittest

from petal_test_support import (
    camera_homography,
    capture,
    captures_fixture,
    luma_of,
    payload,
    rendered,
)

from iroha_petal import (
    FINDER_CENTERS,
    TRACK_WINDOW_MS,
    DecodeErrorKind,
    Luma,
    ScanLimits,
    ScanSession,
)

WIDTH, HEIGHT = 640, 480


def camera_frame(data: bytes, kind: int, frame: int, rotation_deg: float = 20.0) -> Luma:
    source = rendered(data, kind, frame, 512, 2).to_luma()
    pose = camera_homography(WIDTH, HEIGHT, 0.8, rotation_deg, tilt=0.15)
    return capture(source, pose, WIDTH, HEIGHT, noise_seed=frame + 1)


class SessionTest(unittest.TestCase):
    def test_a_steady_camera_is_tracked_after_the_first_frame(self) -> None:
        data = payload(300, 3)
        session = ScanSession(ScanLimits())
        for frame in range(6):
            outcome = session.push(camera_frame(data, 2, frame, rotation_deg=8.0), frame * 125)
            self.assertIsNone(outcome.error, f"frame {frame}")
        stats = session.stats()
        self.assertEqual(stats.readable, 6)
        self.assertEqual(stats.tracked, 5, "every frame after the first follows the pose")
        self.assertEqual(stats.inferred, 0)
        # a pause longer than the tracking window forces a full search again
        session.push(camera_frame(data, 2, 6, rotation_deg=8.0), 5 * 125 + TRACK_WINDOW_MS + 1)
        self.assertEqual(session.stats().tracked, 5)
        self.assertEqual(session.stats().readable, 7)
        # a reset forgets the pose as well
        session.reset()
        session.push(camera_frame(data, 2, 7, rotation_deg=8.0), 5 * 125 + TRACK_WINDOW_MS + 2)
        self.assertEqual(session.stats().tracked, 5)
        self.assertEqual(session.stats().readable, 8)

    def test_a_hidden_corner_blossom_is_counted_and_followed(self) -> None:
        capture_entry = next(
            c for c in captures_fixture()["captures"] if c["name"] == "hidden-corner-540p"
        )
        image = luma_of(capture_entry)
        session = ScanSession()
        outcome = session.push(image, 0)
        self.assertEqual(outcome.lanes, capture_entry["reference_decoded"])
        self.assertEqual((session.stats().inferred, session.stats().tracked), (1, 0))
        # the next frame follows the pose; the blossom is still hidden
        outcome = session.push(image, 100)
        self.assertEqual(outcome.lanes, capture_entry["reference_decoded"])
        self.assertEqual((session.stats().inferred, session.stats().tracked), (2, 1))

    def test_a_session_receives_a_payload_from_simulated_captures(self) -> None:
        data = payload(500, 3)
        session = ScanSession(ScanLimits())
        done = None
        for frame in range(40):
            outcome = session.push(camera_frame(data, 2, frame), frame * 125)
            self.assertIsNone(outcome.error)
            if outcome.completed is not None:
                done = outcome.completed
                break
        self.assertIsNotNone(done, "completed")
        self.assertEqual(done.payload, data)
        self.assertEqual(done.meta.kind, 2)
        stats = session.stats()
        self.assertGreater(stats.readable, 0)
        self.assertGreater(stats.lane_d, 0)
        self.assertEqual(stats.frames, frame + 1)
        self.assertEqual(stats.located, stats.frames)
        self.assertTrue(session.progress().complete)

    def test_idle_sessions_forget_partial_streams(self) -> None:
        data = payload(4_000, 3)
        session = ScanSession(ScanLimits(idle_timeout_ms=1_000))
        session.push(camera_frame(data, 1, 0, rotation_deg=0.0), 0)
        self.assertGreater(session.progress().rank, 0)
        # a frame much later with nothing readable resets the session first
        outcome = session.push(Luma(WIDTH, HEIGHT), 60_000)
        self.assertIs(outcome.error, DecodeErrorKind.NO_FINDERS)
        self.assertEqual(outcome.progress.rank, 0)
        self.assertIsNone(outcome.progress.meta)

    def test_located_counts_codes_that_were_seen_but_could_not_be_read(self) -> None:
        frame = rendered(payload(100, 3), 1, 1, 512, 2).to_luma()
        # keep only the four blossoms: finders are located, no lane can be read
        scale = 512.0 / 1024.0
        centres = [(fx * scale, fy * scale) for fx, fy in FINDER_CENTERS]
        data = bytearray(frame.data)
        for y in range(512):
            for x in range(512):
                if not any(math.hypot(x - fx, y - fy) < 34.0 for fx, fy in centres):
                    data[y * 512 + x] = 0
        session = ScanSession(ScanLimits())
        outcome = session.push(Luma(512, 512, bytes(data)), 0)
        self.assertIs(outcome.error, DecodeErrorKind.NO_ORIENTATION)
        self.assertEqual(session.stats().located, 1)
        self.assertEqual(session.stats().readable, 0)
        # a frame with no code at all is not "located"
        session.push(Luma(320, 240), 100)
        self.assertEqual(session.stats().located, 1)
        self.assertEqual(session.stats().frames, 2)

    def test_unreadable_frames_do_not_disturb_progress(self) -> None:
        session = ScanSession()
        outcome = session.push(Luma(320, 240), 5)
        self.assertIsNone(outcome.completed)
        self.assertEqual(outcome.lanes, "")
        self.assertEqual(session.stats().frames, 1)
        self.assertEqual(session.stats().located, 0)
        outcome = session.push(Luma(10, 10), 6)
        self.assertIs(outcome.error, DecodeErrorKind.UNSUPPORTED_IMAGE)

    def test_absolute_timeout_restarts_a_slow_stream(self) -> None:
        data = payload(4_000, 4)
        session = ScanSession(ScanLimits(absolute_timeout_ms=500))
        session.push(camera_frame(data, 1, 0, rotation_deg=0.0), 0)
        self.assertIsNotNone(session.progress().meta)
        # frame 2 carries no beacon: after the reset its atoms wait for one
        outcome = session.push(camera_frame(data, 1, 2, rotation_deg=0.0), 1_000)
        self.assertEqual(outcome.lanes, "PKD")
        self.assertIsNone(outcome.progress.meta)
        self.assertEqual(outcome.progress.rank, 0)

    def test_camera_planes_with_row_padding_are_accepted(self) -> None:
        data = payload(300, 5)
        image = camera_frame(data, 1, 4, rotation_deg=-35.0)
        stride = WIDTH + 64
        plane = b"".join(
            image.data[y * WIDTH : (y + 1) * WIDTH] + b"\x7f" * 64 for y in range(HEIGHT)
        )
        session = ScanSession()
        outcome = session.push_plane(WIDTH, HEIGHT, stride, plane, 0)
        self.assertIsNone(outcome.error)
        self.assertIn("D", outcome.lanes)
        self.assertEqual(outcome.progress.meta.length, 300)
        with self.assertRaises(ValueError):
            session.push_plane(WIDTH, HEIGHT, WIDTH - 1, plane, 1)


if __name__ == "__main__":
    unittest.main()
