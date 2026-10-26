# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Golden camera captures in ``fixtures/petal/petal_captures_v5.json``.

Every conforming decoder must read the lanes named in ``must_decode``, the turn
lanes named in ``must_turn`` and the kana lanes named in ``must_kana``, must
never report wrong data for any lane, must report the recorded inferred corner,
must follow each tracking pair from its first frame into its second, must
recover the ``must_merge`` field lanes of every merge set from the merge of its
captures, must never let a session offer a lane of a mixed set, and must reject
the negatives. This port follows the reference operation by operation, so it is
also held to the exact lanes the reference read (``reference_decoded``,
``reference_turned``, ``reference_kana``, ``reference_tracked`` and
``reference_merged``).
"""

from __future__ import annotations

import sys
import time
import unittest

from petal_test_support import (
    captures_fixture,
    letters,
    luma_of,
    payload,
    read_lanes,
    stream_fixture,
    truth,
)

from iroha_petal import (
    DECODE_ORDER,
    KANA_LANES,
    Beacon,
    DecodeError,
    LaneHeader,
    RenderOptions,
    ScanSession,
    StreamAssembler,
    StreamEncoder,
    decode_frame,
    decode_lane,
    encode_lane,
    field_evidence,
    kana_packet,
    merge_fields,
    read_kana,
    read_turns,
    render_frame,
    track_frame,
    turn_packet,
)

#: Generous per-capture budget for plain CPython (the port takes well under a second).
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
    "barrel-480p",
    "upright-720p",
    "upright-legacy-720p",
    "upright-soft-720p",
    "upright-480p",
]

#: The tracking pairs of the fixture, in order.
TRACK_NAMES = ["steady-hand-540p", "thumb-arrives-540p"]


def read_turn_lanes(test: unittest.TestCase, name: str, image, frame, entry: dict, assembler=None):
    """Reads the turn lanes of a decoded capture, checks them against the recorded data and
    the requirements, and returns their sector digits and how long the read took; their
    atoms go to ``assembler`` when given."""
    expected = [bytes.fromhex(data) for data in entry["turn_data"]]
    started = time.perf_counter()
    results = read_turns(image, frame)
    elapsed = time.perf_counter() - started
    letters = ""
    packets = []
    for sector, result in enumerate(results):
        if result is None:
            continue
        test.assertEqual(result.data, expected[sector], f"{name}: turn lane {sector} data")
        field = frame.fields[sector]
        test.assertIsNotNone(field, f"{name}: a turn lane rides on its field lane")
        header = LaneHeader(field.data[0], (field.data[1] << 8) | field.data[2])
        packet = turn_packet(header, sector, result.data)
        test.assertIsNotNone(packet, f"{name}: turn check {sector}")
        packets.append(packet)
        letters += str(sector)
    if assembler is not None:
        for packet in packets:
            assembler.push_atoms(packet)
    for letter in entry["must_turn"]:
        test.assertIn(letter, letters, f"{name}: turned {letters!r}, must turn {letter}")
    test.assertEqual(letters, entry["reference_turned"], f"{name}: turn lanes read")
    return letters, elapsed


def kana_header(frame) -> LaneHeader:
    """The header (stream tag and frame) a decoded capture's lanes name: its first atom
    packet's, or its beacon's."""
    packets = frame.atom_packets()
    if packets:
        return packets[0].header
    beacon = frame.beacon()
    assert isinstance(beacon, Beacon), "a decoded capture names its frame"
    return beacon.header


def read_kana_lanes(test: unittest.TestCase, name: str, image, frame, entry: dict, assembler=None):
    """Reads the kana lanes of a decoded capture under the header its lanes name (both bands),
    checks them against the recorded data and the requirements, and returns their names and
    how long the read took; their atoms go to ``assembler`` when given."""
    expected = [bytes.fromhex(data) for data in entry.get("kana_data", [])]
    started = time.perf_counter()
    results = read_kana(image, frame, kana_header(frame))
    elapsed = time.perf_counter() - started
    names = ""
    packets = []
    for k, (lane, result) in enumerate(zip(KANA_LANES, results)):
        if result is None:
            continue
        if expected:
            test.assertEqual(result.result.data, expected[k], f"{name}: kana lane {lane.value}")
        packet = kana_packet(result.header, lane, result.result.data)
        test.assertIsNotNone(packet, f"{name}: kana check {lane.value}")
        packets.append(packet)
        names += lane.value
    if assembler is not None:
        for packet in packets:
            assembler.push_atoms(packet)
    must = entry.get("must_kana", "")
    for required in (must[i : i + 2] for i in range(0, len(must), 2)):
        test.assertIn(
            required,
            [names[i : i + 2] for i in range(0, len(names), 2)],
            f"{name}: kana {names!r}, must read {must!r}",
        )
    test.assertEqual(names, entry["reference_kana"], f"{name}: kana lanes read")
    return names, elapsed


def merge_set(test: unittest.TestCase, entry: dict):
    """Decodes every capture of a merge set as recorded and pushes it through a fresh
    session (30 fps); returns the merge of their field evidence, the lanes the session
    merged and its counters."""
    name = entry["name"]
    recorded = truth(entry)
    evidence = []
    session = ScanSession()
    merged = ""
    for i, capture in enumerate(entry["captures"]):
        image = luma_of({**entry, **capture})
        try:
            decoded = decode_frame(image)
        except DecodeError as error:  # pragma: no cover - failure path
            test.fail(f"{name} capture {i}: {error}")
        read = read_lanes(test, f"{name} capture {i}", decoded, recorded)
        for letter in capture["must_decode"]:
            test.assertIn(letter, read, f"{name} capture {i}: must read {letter}")
        test.assertEqual(read, capture["reference_decoded"], f"{name} capture {i}: lanes")
        found = field_evidence(image, decoded)
        test.assertIsNotNone(found, f"{name} capture {i}: field evidence")
        evidence.append(found)
        merged += session.push(image, 33 * i).merged
    return merge_fields(evidence), merged, session.stats()


def merged_letters(test: unittest.TestCase, name: str, results, entry: dict) -> str:
    """Checks merged field lanes against the recorded data and returns their sector
    digits."""
    recorded = truth(entry)
    for lane, result in zip(DECODE_ORDER[1:], results):
        if result is not None:
            test.assertEqual(result.data, recorded[lane], f"{name}: merged lane {lane.letter}")
    return letters(results)


class CaptureTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.doc = captures_fixture()

    def test_the_fixture_carries_its_captures_tracks_merges_and_negatives(self) -> None:
        self.assertEqual(self.doc["fixture_version"], 5)
        self.assertEqual(self.doc["format"], "petal-captures-v5")
        self.assertEqual([c["name"] for c in self.doc["captures"]], CAPTURE_NAMES)
        self.assertEqual([t["name"] for t in self.doc["tracks"]], TRACK_NAMES)
        self.assertEqual([m["name"] for m in self.doc["merges"]], ["noisy-480p"])
        self.assertEqual([m["name"] for m in self.doc["mixed_merges"]], ["mixed-frames-480p"])
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
            self.assertLess(elapsed, TIME_BUDGET_S, f"{name}: decode took {elapsed:.1f} s")
            self.assertEqual(decoded.mirrored, capture["mirrored"], f"{name}: mirror flag")
            read = read_lanes(self, name, decoded, truth(capture))
            for letter in capture["must_decode"]:
                self.assertIn(letter, read, f"{name}: read {read!r}, must read {letter}")
            self.assertEqual(read, capture["reference_decoded"], f"{name}: lanes read")
            self.assertEqual(
                decoded.inferred_corner, capture["inferred_corner"], f"{name}: inferred corner"
            )
            decoded.feed(assembler)
            turned, turn_elapsed = read_turn_lanes(self, name, image, decoded, capture, assembler)
            self.assertLess(turn_elapsed, TIME_BUDGET_S, f"{name}: turns took {turn_elapsed:.1f} s")
            kana, kana_elapsed = read_kana_lanes(self, name, image, decoded, capture, assembler)
            self.assertLess(kana_elapsed, TIME_BUDGET_S, f"{name}: bands took {kana_elapsed:.1f} s")
            timings.append(
                (
                    name,
                    image.width,
                    image.height,
                    read,
                    turned,
                    kana,
                    decoded.pose.distortion,
                    elapsed,
                    turn_elapsed,
                    kana_elapsed,
                )
            )
        # captures of different frames of the same stream accumulate in one assembler
        self.assertGreater(assembler.progress().atoms_received, 100)
        for name, width, height, read, turned, kana, distortion, *seconds in timings:
            sys.stderr.write(
                f"\n  {name:<22} {width}x{height} lanes {read:<5} turns {turned:<4} "
                f"kana {kana:<8} k {distortion:+.3f} "
                + " + ".join(f"{elapsed:.2f} s" for elapsed in seconds)
            )
        sys.stderr.write("\n")

    def test_the_golden_turn_lanes_cover_the_cameras_the_spec_names(self) -> None:
        turned = {c["name"]: c["reference_turned"] for c in self.doc["captures"]}
        # the sharp cameras, the mirrored, covered and cut-off captures and the sharp upright
        # phone read every turn lane; the older upright phone three, the over-exposed capture
        # one; the soft, blurred, veiled and distorted none
        for name in (
            "clean-512",
            "modern-720p-rotated",
            "selfie-mirrored-540p",
            "hidden-corner-540p",
            "cut-corner-720p",
            "shadow-band-540p",
            "upright-720p",
        ):
            self.assertEqual(turned[name], "0123", name)
        self.assertEqual(turned["upright-legacy-720p"], "123")
        self.assertEqual(turned["overexposed-540p"], "1")
        for name in (
            "legacy-540p-tilted",
            "soft-480p-blur1.9",
            "small-480p",
            "veiled-720p",
            "barrel-480p",
            "upright-soft-720p",
            "upright-480p",
        ):
            self.assertEqual(turned[name], "", name)
        for pair in self.doc["tracks"]:
            self.assertEqual(pair["reference_turned"], "0123", pair["name"])
        by_name = {c["name"]: c for c in self.doc["captures"]}
        # the shadow band reads its turn lanes with 3 to 9 corrections, exactly as the
        # reference reads them
        image = luma_of(by_name["shadow-band-540p"])
        results = read_turns(image, decode_frame(image))
        self.assertEqual(
            [(r.corrected, r.erasures) for r in results], [(5, 0), (6, 0), (3, 0), (9, 0)]
        )
        # the over-exposed capture reads one with 10 corrections of its 11, as the reference
        image = luma_of(by_name["overexposed-540p"])
        results = read_turns(image, decode_frame(image))
        self.assertEqual([(r.corrected, r.erasures) for r in results if r is not None], [(10, 0)])

    def test_the_golden_kana_lanes_cover_the_cameras_the_spec_names(self) -> None:
        kana = {c["name"]: c["reference_kana"] for c in self.doc["captures"]}
        # the phones held upright see both bands: the sharp, older and soft ones read all four
        # kana lanes, the 480p one P0, P1 and K1; the clean card and the veiled capture read
        # all four too, and the tilted captures the band in view
        for name in ("upright-720p", "upright-legacy-720p", "upright-soft-720p"):
            self.assertEqual(kana[name], "P0K0P1K1", name)
        self.assertEqual(kana["upright-480p"], "P0P1K1")
        self.assertEqual((kana["clean-512"], kana["veiled-720p"]), ("P0K0P1K1", "P0K0P1K1"))
        self.assertEqual((kana["modern-720p-rotated"], kana["cut-corner-720p"]), ("P1K1", "P0K0"))
        must = {c["name"]: c["must_kana"] for c in self.doc["captures"] if c["must_kana"]}
        self.assertEqual(
            must,
            {
                "upright-720p": "P0K0P1K1",
                "upright-legacy-720p": "P0P1",
                "upright-soft-720p": "P0P1",
            },
        )
        for pair in self.doc["tracks"]:
            self.assertEqual(pair["reference_kana"], "", pair["name"])
        by_name = {c["name"]: c for c in self.doc["captures"]}
        # every lane is read under the square's own frame, with the corrections and erasures
        # the reference reads it with
        for name, expected in (
            ("upright-legacy-720p", [(0, 0), (15, 0), (0, 0), (10, 0)]),
            ("upright-480p", [(12, 8), None, (0, 0), (0, 0)]),
            ("overexposed-540p", [None, None, (12, 8), None]),
        ):
            capture = by_name[name]
            image = luma_of(capture)
            frame = decode_frame(image)
            results = read_kana(image, frame, kana_header(frame))
            self.assertEqual(
                [None if r is None else (r.result.corrected, r.result.erasures) for r in results],
                expected,
                name,
            )
            for result in results:
                if result is not None:
                    self.assertEqual(
                        result.header, LaneHeader(capture_tag(self.doc), capture["frame"])
                    )

    def test_the_cheap_lens_needs_the_distortion_search(self) -> None:
        by_name = {c["name"]: c for c in self.doc["captures"]}
        decoded = decode_frame(luma_of(by_name["barrel-480p"]))
        self.assertLess(decoded.pose.distortion, 0.0)
        self.assertEqual(decode_frame(luma_of(by_name["clean-512"])).pose.distortion, 0.0)

    def test_golden_tracks_follow_the_pose_into_the_next_frame(self) -> None:
        timings = []
        for pair in self.doc["tracks"]:
            name = pair["name"]
            try:
                previous = decode_frame(luma_of(pair, "from_luma_zlib_base64"))
            except DecodeError as error:  # pragma: no cover - failure path
                self.fail(f"{name}: first frame: {error}")
            started = time.perf_counter()
            followed = track_frame(luma_of(pair, "to_luma_zlib_base64"), previous)
            elapsed = time.perf_counter() - started
            self.assertIsNotNone(followed, f"{name}: tracking lost the code")
            read = read_lanes(self, name, followed, truth(pair))
            for letter in pair["must_track"]:
                self.assertIn(letter, read, f"{name}: tracked {read!r}, must track {letter}")
            self.assertEqual(read, pair["reference_tracked"], f"{name}: lanes tracked")
            self.assertEqual(
                followed.inferred_corner, pair["inferred_corner"], f"{name}: inferred corner"
            )
            # the orientation is kept from the first frame
            self.assertEqual(
                (followed.rotation, followed.mirrored), (previous.rotation, previous.mirrored)
            )
            turned, turn_elapsed = read_turn_lanes(
                self, name, luma_of(pair, "to_luma_zlib_base64"), followed, pair
            )
            read_kana_lanes(self, name, luma_of(pair, "to_luma_zlib_base64"), followed, pair)
            timings.append((name, read, turned, elapsed, turn_elapsed))
        for name, read, turned, elapsed, turn_elapsed in timings:
            sys.stderr.write(
                f"\n  {name:<22} tracked lanes {read:<5} turns {turned:<4} {elapsed:.2f} s "
                f"+ {turn_elapsed:.2f} s"
            )
        sys.stderr.write("\n")

    def test_golden_merge_sets_recover_lanes_no_capture_reads_alone(self) -> None:
        for entry in self.doc["merges"]:
            name = entry["name"]
            results, session_merged, stats = merge_set(self, entry)
            merged = merged_letters(self, name, results, entry)
            self.assertEqual(merged, entry["reference_merged"], f"{name}: merged lanes")
            for letter in entry["must_merge"]:
                self.assertIn(
                    letter,
                    session_merged,
                    f"{name}: the session did not recover lane {letter} "
                    f"(merged {session_merged!r})",
                )
            self.assertGreater(stats.merges, 0, f"{name}: no merge")
            self.assertEqual(stats.merged_lanes, len(session_merged), name)

    def test_golden_mixed_sets_never_offer_the_other_frames_lanes(self) -> None:
        for entry in self.doc["mixed_merges"]:
            name = entry["name"]
            results, session_merged, stats = merge_set(self, entry)
            # the merge itself reads the petal field, which shows another frame than the rings
            merged = merged_letters(self, name, results, entry)
            rings_frame = bytes.fromhex(entry["d_data"])[1:3]
            for result in results:
                if result is not None:
                    self.assertNotEqual(
                        result.data[1:3], rings_frame, f"{name}: a merged lane names the rings"
                    )
                    self.assertEqual(
                        int.from_bytes(result.data[1:3], "big"), entry["petals_frame"], name
                    )
            self.assertEqual(merged, entry["reference_merged"], f"{name}: merged lanes")
            self.assertGreater(stats.merges, 0, f"{name}: no merge was tried")
            self.assertEqual(session_merged, "", f"{name}: the session offered another frame")
            self.assertEqual(stats.merged_lanes, 0, name)

    def test_negative_captures_are_rejected(self) -> None:
        for negative in self.doc["negatives"]:
            with self.assertRaises(DecodeError, msg=negative["name"]):
                decode_frame(luma_of(negative))

    def test_recorded_lane_data_is_a_valid_codeword_of_its_stream(self) -> None:
        for capture in self.doc["captures"]:
            for lane, data in truth(capture).items():
                word = encode_lane(lane, data)
                self.assertEqual(decode_lane(lane, word), data, f"{capture['name']} {lane}")

    def test_captures_show_the_one_pass_stream(self) -> None:
        stream = next(s for s in stream_fixture()["streams"] if s["name"] == "one-pass")
        data = bytes.fromhex(self.doc["payload_hex"])
        self.assertEqual(data, bytes.fromhex(stream["payload_hex"]))
        self.assertEqual(data, payload(3000, 2))
        encoder = StreamEncoder(data, self.doc["payload_kind"])

        def recorded(frame: int) -> dict:
            lanes = encoder.lane_data(frame)
            return {
                "d_data": lanes.d.hex(),
                "field_data": [f.hex() for f in lanes.fields],
                "turn_data": [t.hex() for t in lanes.turns],
                "kana_data": [k.hex() for k in lanes.kana],
            }

        for capture in self.doc["captures"]:
            self.assertEqual(
                {key: capture[key] for key in ("d_data", "field_data", "turn_data", "kana_data")},
                recorded(capture["frame"]),
                capture["name"],
            )
        for pair in self.doc["tracks"]:
            lanes = recorded(pair["to_frame"])
            self.assertEqual(
                {key: pair[key] for key in ("d_data", "field_data", "turn_data")},
                {key: lanes[key] for key in ("d_data", "field_data", "turn_data")},
                pair["name"],
            )
        for entry in self.doc["merges"]:
            lanes = recorded(entry["frame"])
            self.assertEqual(
                {key: entry[key] for key in ("d_data", "field_data")},
                {key: lanes[key] for key in ("d_data", "field_data")},
                entry["name"],
            )
        for entry in self.doc["mixed_merges"]:
            rings, petals = recorded(entry["frame"]), recorded(entry["petals_frame"])
            self.assertEqual(entry["d_data"], rings["d_data"], entry["name"])
            self.assertEqual(entry["field_data"], petals["field_data"], entry["name"])

    def test_renderer_reproduces_the_clean_capture_pixel_for_pixel(self) -> None:
        # the clean capture is the whole card, its bands included
        capture = next(c for c in self.doc["captures"] if c["name"] == "clean-512")
        encoder = StreamEncoder(bytes.fromhex(self.doc["payload_hex"]), self.doc["payload_kind"])
        rgb = render_frame(encoder.cells(capture["frame"]), RenderOptions(size=512, supersample=3))
        self.assertEqual((rgb.width, rgb.height), (capture["width"], capture["height"]))
        self.assertEqual(rgb.to_luma().data, luma_of(capture).data)


def capture_tag(doc: dict) -> int:
    """The stream tag of the captures' payload: the low byte of its CRC-32C."""
    return StreamEncoder(bytes.fromhex(doc["payload_hex"]), doc["payload_kind"]).meta.tag


if __name__ == "__main__":
    unittest.main()
