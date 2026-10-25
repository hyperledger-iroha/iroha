# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Merging the captures of one shown frame: golden recovery, wrong-frame refusal and
bounded session state."""

from __future__ import annotations

import contextlib
import functools
import io
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from petal_test_support import captures_fixture, letters, luma_of, payload, truth

from iroha_petal import (
    FIELD_LANES,
    DecodeOptions,
    LaneResult,
    Luma,
    RenderOptions,
    ScanSession,
    StreamAssembler,
    StreamEncoder,
    decode_frame,
    field_evidence,
    merge_fields,
    render_frame,
)
from iroha_petal.cli import main
from iroha_petal.pngio import write_image
from iroha_petal.session import MERGE_CAPTURES, MERGE_WINDOW

NO_FIELDS = (None, None, None, None)


@functools.lru_cache(maxsize=None)
def one_pass() -> StreamEncoder:
    """The ``one-pass`` stream of the golden captures."""
    return StreamEncoder(payload(3000, 2), 2)


@functools.lru_cache(maxsize=None)
def source(encoder: StreamEncoder, frame: int) -> Luma:
    """A 512-pixel render of ``frame``."""
    cells = encoder.cells(frame)
    return render_frame(cells, RenderOptions(size=512, supersample=2)).to_luma()


@functools.lru_cache(maxsize=None)
def decoded_source(encoder: StreamEncoder, frame: int):
    return decode_frame(source(encoder, frame))


def rings_of_another_frame(petals: int, rings: int):
    """A clean capture of frame ``petals`` whose decoded lanes are replaced by lane ``D`` of
    frame ``rings``, as if the rings showed another frame than the petal field."""
    encoder = one_pass()
    image = source(encoder, petals)
    frame = decoded_source(encoder, petals)
    assert frame.lanes == "D0123"
    d = LaneResult(encoder.lane_data(rings).d, 0, 0)
    return image, replace(frame, d=d, fields=NO_FIELDS)


class MergeTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.fixture = captures_fixture()
        cls.options = DecodeOptions()

    def test_averaging_recovers_recorded_lanes_that_no_capture_reads(self) -> None:
        for entry in self.fixture["merges"]:
            with self.subTest(name=entry["name"]):
                recorded = truth(entry)
                evidence = []
                for capture in entry["captures"]:
                    image = luma_of({**entry, **capture})
                    frame = decode_frame(image)
                    self.assertEqual(frame.lanes, capture["reference_decoded"])
                    for letter in entry["must_merge"]:
                        self.assertIsNone(frame.fields[int(letter)])
                    evidence.append(field_evidence(image, frame, self.options))
                merged = merge_fields(evidence)
                self.assertEqual(letters(merged), entry["reference_merged"])
                for lane, result in zip(FIELD_LANES, merged):
                    if result is not None:
                        self.assertEqual(result.data, recorded[lane])

    def test_sessions_offer_each_recovered_lane_once(self) -> None:
        for entry in self.fixture["merges"]:
            with self.subTest(name=entry["name"]):
                session = ScanSession()
                recovered = ""
                for i, capture in enumerate(entry["captures"]):
                    outcome = session.push(luma_of({**entry, **capture}), 33 * i)
                    self.assertIsNone(outcome.error)
                    recovered += outcome.merged
                for letter in entry["must_merge"]:
                    self.assertEqual(recovered.count(letter), 1)
                self.assertEqual(len(set(recovered)), len(recovered))
                stats = session.stats()
                # no capture reads a field lane alone, so every one came from merging
                self.assertEqual(stats.field_lanes, 0)
                self.assertEqual(stats.merged_lanes, len(recovered))
                self.assertGreater(stats.merges, 0)
                # the merged atoms wait for the beacon of their stream
                self.assertIsNone(session.progress().meta)
                self.assertEqual(session.progress().atoms_received, 0)
                session.push(source(one_pass(), 20), 500)
                self.assertEqual(session.progress().meta, one_pass().meta)
                self.assertGreaterEqual(session.progress().atoms_received, 5 * len(recovered))

    def test_mixed_frame_field_lanes_never_enter_the_assembler(self) -> None:
        for entry in self.fixture["mixed_merges"]:
            with self.subTest(name=entry["name"]):
                session = ScanSession()
                baseline = StreamAssembler()
                evidence = []
                for i, capture in enumerate(entry["captures"]):
                    image = luma_of({**entry, **capture})
                    frame = decode_frame(image)
                    self.assertEqual(frame.lanes, capture["reference_decoded"])
                    frame.feed(baseline)
                    evidence.append(field_evidence(image, frame, self.options))
                    self.assertEqual(session.push(image, 33 * i).merged, "")
                merged = merge_fields(evidence)
                self.assertEqual(letters(merged), entry["reference_merged"])
                for result in merged:
                    if result is not None:
                        self.assertEqual(
                            int.from_bytes(result.data[1:3], "big"), entry["petals_frame"]
                        )
                self.assertEqual(session.progress(), baseline.progress())
                self.assertEqual(session.stats().merged_lanes, 0)
                self.assertGreater(session.stats().merges, 0)

    def test_one_capture_reads_what_decode_read_and_invalid_evidence_is_refused(self) -> None:
        names = (
            "clean-512",
            "shadow-band-540p",
            "hidden-corner-540p",
            "selfie-mirrored-540p",
            "barrel-480p",
        )
        for name in names:
            entry = next(c for c in self.fixture["captures"] if c["name"] == name)
            image = luma_of(entry)
            frame = decode_frame(image)
            evidence = field_evidence(image, frame, self.options)
            self.assertEqual(merge_fields([evidence]), frame.fields, name)
            self.assertIsNone(field_evidence(Luma(20, 20), frame, self.options))
            self.assertIsNone(field_evidence(image, replace(frame, inferred_corner=7)))
        self.assertEqual(merge_fields([]), NO_FIELDS)

    def test_command_line_reports_recovered_lanes(self) -> None:
        entry = self.fixture["merges"][0]
        out, err = io.StringIO(), io.StringIO()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            frames = root / "frames"
            frames.mkdir()
            for i, capture in enumerate(entry["captures"]):
                write_image(frames / f"{i:02}.pgm", luma_of({**entry, **capture}))
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                status = main(
                    ["decode", "--input-dir", str(frames), "--output", str(root / "out.bin")]
                )
        self.assertEqual(status, 1)  # these captures contain one shown frame, not a whole stream
        # the second capture's merge recovers lane 0, the third's lane 1, the last's 2 and 3
        # (as the reference session reads these captures)
        self.assertIn("01.pgm: D (tracked) (merged 0);", err.getvalue())
        self.assertIn("02.pgm: D (tracked) (merged 1);", err.getvalue())
        self.assertIn("03.pgm: D (tracked) (merged 23);", err.getvalue())
        self.assertIn("0 field lanes, 0 turn lanes; 3 merges, 4 merged field lanes", err.getvalue())

    def test_a_merge_never_offers_atoms_of_another_shown_frame(self) -> None:
        session = ScanSession()
        for _ in range(3):
            image, frame = rings_of_another_frame(11, 10)
            self.assertEqual(session._merge(image, frame), "")
        stats = session.stats()
        self.assertEqual((stats.merges, stats.merged_lanes), (2, 0))
        self.assertEqual(session.progress().atoms_received, 0)
        # what was refused: the merged field lanes name frame 11, not frame 10
        held = session._held[0]
        self.assertEqual(held.shown.frame, 10)
        merged = merge_fields(list(held.evidence))
        expected = one_pass().lane_data(11).fields
        self.assertEqual(tuple(r.data if r else None for r in merged), expected)

    def test_torn_captures_whose_lanes_name_different_frames_are_not_held(self) -> None:
        session = ScanSession()
        image, frame = rings_of_another_frame(11, 10)
        fields = list(frame.fields)
        fields[2] = LaneResult(one_pass().lane_data(11).fields[2], 0, 0)
        self.assertEqual(session._merge(image, replace(frame, fields=tuple(fields))), "")
        self.assertEqual(len(session._held), 0)
        # nor is a capture that read nothing
        self.assertEqual(session._merge(image, replace(frame, d=None)), "")
        self.assertEqual(len(session._held), 0)

    def test_the_session_holds_at_most_four_captures_of_six_shown_frames(self) -> None:
        session = ScanSession()
        # refused merges never finish a shown frame, so its captures stay held
        for _ in range(MERGE_CAPTURES + 2):
            image, frame = rings_of_another_frame(11, 10)
            session._merge(image, frame)
        self.assertEqual(len(session._held), 1)
        self.assertEqual(len(session._held[0].evidence), MERGE_CAPTURES)
        self.assertEqual(session.stats().merges, MERGE_CAPTURES + 1)
        for rings in range(12, 12 + MERGE_WINDOW):
            image, frame = rings_of_another_frame(11, rings)
            session._merge(image, frame)
        self.assertEqual(len(session._held), MERGE_WINDOW)
        self.assertTrue(all(h.shown.frame >= 12 for h in session._held))
        self.assertTrue(all(len(h.evidence) <= MERGE_CAPTURES for h in session._held))
        session.reset()
        self.assertEqual(len(session._held), 0)
        # a receive timeout forgets the held evidence with the stream
        image, frame = rings_of_another_frame(11, 10)
        session._merge(image, frame)
        self.assertEqual(len(session._held[0].evidence), 1)
        session.push(source(one_pass(), 0), 0)
        self.assertIsNotNone(session.progress().meta)
        session.push(Luma(320, 240), 60_000)
        self.assertEqual(len(session._held), 0, "timeout forgets the previous stream's evidence")

    def test_switching_streams_with_the_same_short_tag_discards_old_merge_state(self) -> None:
        def encoder(last: int) -> StreamEncoder:
            data = bytearray(100)
            data[3] = last
            return StreamEncoder(bytes(data), 2)

        old, new = encoder(49), encoder(128)
        self.assertEqual((old.meta.tag, new.meta.tag), (61, 61))
        self.assertNotEqual(old.meta, new.meta)
        old_image = source(old, 0)
        new_image = source(new, 0)
        old_full = decode_frame(old_image)
        new_partial = replace(decode_frame(new_image), fields=NO_FIELDS)
        for was_complete in (False, True):
            session = ScanSession()
            previous = old_full if was_complete else replace(old_full, fields=NO_FIELDS)
            session._absorb(previous)
            session._merge(old_image, previous)
            self.assertNotEqual(len(session._held), 0)
            session._absorb(new_partial)
            session._absorb(new_partial)
            self.assertEqual(session.progress().meta, new.meta)
            self.assertEqual(len(session._held), 0)
            self.assertEqual(session._merge(new_image, new_partial), "")
            self.assertEqual(len(session._held[0].evidence), 1)
            self.assertFalse(any(session._held[0].done))
            self.assertEqual(session._merge(new_image, new_partial), "0123")
            self.assertEqual(session._merge(new_image, new_partial), "")

    def test_the_first_beacon_preserves_evidence_received_before_it(self) -> None:
        encoder = one_pass()
        image = source(encoder, 1)
        partial = replace(decode_frame(image), fields=NO_FIELDS)
        session = ScanSession()
        session._absorb(partial)
        session._merge(image, partial)
        self.assertIsNone(session.progress().meta)
        evidence = session._held[0].evidence[0]
        beacon = replace(decode_frame(source(encoder, 0)), fields=NO_FIELDS)
        session._absorb(beacon)
        self.assertEqual(session.progress().meta, encoder.meta)
        self.assertIs(session._held[0].evidence[0], evidence)
        self.assertEqual(session._merge(image, partial), "0123")


if __name__ == "__main__":
    unittest.main()
