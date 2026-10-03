# Copyright 2026 Hyperledger Iroha Contributors
# SPDX-License-Identifier: Apache-2.0

"""Stream encoder, beacons, atom ids and the receiver-side assembler."""

from __future__ import annotations

import unittest

from petal_test_support import payload

from iroha_petal.lanes import ATOM_LEN, Lane, decode_lane
from iroha_petal.prng import Xorshift32
from iroha_petal.stream import (
    BEACON_INTERVAL,
    MAX_PAYLOAD_LEN,
    AssemblerLimits,
    AtomPacket,
    Beacon,
    LaneHeader,
    StreamAssembler,
    StreamEncoder,
    StreamError,
    StreamErrorKind,
    _lane_first_id,
    atoms_in_frame,
    first_atom_id,
    is_beacon_frame,
    parse_atom_lane,
    parse_d_lane,
)


def feed_frame(assembler: StreamAssembler, encoder: StreamEncoder, frame: int, lanes) -> None:
    p, k, d = encoder.words(frame)
    for lane in lanes:
        word = {Lane.P: p, Lane.K: k, Lane.D: d}[lane]
        data = decode_lane(lane, word)
        if lane is Lane.D:
            parsed = parse_d_lane(data)
            assert parsed is not None
            assembler.push_d_lane(parsed)
        else:
            packet = parse_atom_lane(lane, data)
            assert packet is not None
            assembler.push_atoms(packet)


ALL = (Lane.D, Lane.P, Lane.K)


class StreamTest(unittest.TestCase):
    def test_atom_ids_are_contiguous_across_frames(self) -> None:
        expected = 0
        for frame in range(71):
            self.assertEqual(first_atom_id(frame), expected, f"frame {frame}")
            expected += atoms_in_frame(frame)
        self.assertEqual(atoms_in_frame(0), 6)
        self.assertEqual(atoms_in_frame(1), 7)
        self.assertEqual(_lane_first_id(Lane.K, 4), first_atom_id(4) + 1)
        self.assertEqual(_lane_first_id(Lane.K, 5), first_atom_id(5) + 2)
        # the frame counter wraps on a beacon frame, so ids repeat cleanly
        self.assertTrue(is_beacon_frame(0) and 65_536 % BEACON_INTERVAL == 0)
        with self.assertRaises(ValueError):
            first_atom_id(65_536)

    def test_clean_stream_completes_after_one_systematic_pass(self) -> None:
        data = payload(1_000, 21)
        encoder = StreamEncoder(data, 2)
        assembler = StreamAssembler(AssemblerLimits())
        for frame in range(encoder.systematic_frames()):
            feed_frame(assembler, encoder, frame, ALL)
        done = assembler.take_completed()
        self.assertIsNotNone(done)
        self.assertEqual(done.payload, data)
        self.assertEqual(done.meta.kind, 2)
        self.assertTrue(assembler.progress().complete)
        self.assertIsNone(assembler.take_completed(), "delivered exactly once")

    def test_any_single_lane_is_enough_given_a_beacon(self) -> None:
        data = payload(400, 22)
        encoder = StreamEncoder(data, 1)
        for lane in (Lane.P, Lane.K, Lane.D):
            assembler = StreamAssembler()
            feed_frame(assembler, encoder, 0, [Lane.D])
            for frame in range(400):
                feed_frame(assembler, encoder, frame, [lane])
                if assembler.progress().complete:
                    break
            done = assembler.take_completed()
            self.assertIsNotNone(done, f"{lane} alone")
            self.assertEqual(done.payload, data)

    def test_atoms_seen_before_the_first_beacon_are_not_lost(self) -> None:
        data = payload(300, 23)
        encoder = StreamEncoder(data, 1)
        assembler = StreamAssembler()
        # frames 1..: no beacon among them until frame 4
        for frame in range(1, 4):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
        self.assertIsNone(assembler.progress().meta)
        for frame in range(4, encoder.systematic_frames() + 4):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
        self.assertEqual(assembler.take_completed().payload, data)

    def test_joining_mid_stream_and_losing_frames_still_completes(self) -> None:
        data = payload(2_500, 24)
        encoder = StreamEncoder(data, 3)
        assembler = StreamAssembler()
        rng = Xorshift32(77)
        frame = 15  # join late
        shown = 0
        while not assembler.progress().complete:
            if rng.next_u32() % 10 < 6:
                # 60 % of the frames are readable
                feed_frame(assembler, encoder, frame, ALL)
            frame = (frame + 1) & 0xFFFF
            shown += 1
            self.assertLess(shown, 600, "stream failed to complete")
        self.assertEqual(assembler.take_completed().payload, data)

    def test_a_different_stream_replaces_the_active_one_after_two_beacons(self) -> None:
        first = StreamEncoder(payload(100, 1), 1)
        second = StreamEncoder(payload(100, 2), 1)
        assembler = StreamAssembler()
        feed_frame(assembler, first, 0, [Lane.D])
        self.assertEqual(assembler.progress().meta, first.meta)
        feed_frame(assembler, second, 0, [Lane.D])
        self.assertEqual(assembler.progress().meta, first.meta)
        feed_frame(assembler, second, 4, [Lane.D])
        self.assertEqual(assembler.progress().meta, second.meta)

    def test_an_interleaved_beacon_of_the_active_stream_cancels_a_switch(self) -> None:
        first = StreamEncoder(payload(100, 1), 1)
        second = StreamEncoder(payload(100, 2), 1)
        assembler = StreamAssembler()
        feed_frame(assembler, first, 0, [Lane.D])
        feed_frame(assembler, second, 0, [Lane.D])
        feed_frame(assembler, first, 4, [Lane.D])
        feed_frame(assembler, second, 8, [Lane.D])
        self.assertEqual(assembler.progress().meta, first.meta)

    def test_a_zero_pending_limit_buffers_nothing(self) -> None:
        data = payload(300, 41)
        encoder = StreamEncoder(data, 1)
        assembler = StreamAssembler(AssemblerLimits(max_pending_atoms=0))
        # atoms before any beacon are dropped, not buffered
        for frame in range(1, 4):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
        self.assertEqual(len(assembler._pending), 0)
        feed_frame(assembler, encoder, 4, [Lane.D])
        self.assertEqual(assembler.progress().rank, 0)
        self.assertEqual(assembler.progress().atoms_received, 0)

    def test_oversized_beacons_are_ignored(self) -> None:
        encoder = StreamEncoder(payload(4_000, 5), 1)
        assembler = StreamAssembler(AssemblerLimits(max_payload_len=1_000))
        feed_frame(assembler, encoder, 0, [Lane.D])
        self.assertIsNone(assembler.progress().meta)

    def test_corrupt_atoms_are_caught_by_the_payload_crc(self) -> None:
        data = payload(200, 25)
        encoder = StreamEncoder(data, 1)
        assembler = StreamAssembler()
        feed_frame(assembler, encoder, 0, (Lane.D, Lane.K))
        # atom 0 arrives with a valid header but a wrong body
        assembler.push_atoms(
            AtomPacket(
                header=LaneHeader(tag=encoder.meta.tag, frame=0),
                first_id=0,
                atoms=(b"\xee" * ATOM_LEN,),
            )
        )
        for frame in range(1, encoder.systematic_frames()):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
        self.assertIsNone(assembler.take_completed())
        self.assertEqual(assembler.progress().integrity_failures, 1)
        # the counter is cumulative: a reset keeps it
        snapshot = assembler.progress()
        assembler.reset()
        self.assertEqual(assembler.progress().integrity_failures, 1)
        feed_frame(assembler, encoder, 0, [Lane.D])
        self.assertEqual(assembler.progress().meta, snapshot.meta)
        # clean repair frames after the reset recover the payload
        for frame in range(100, 600):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
            if assembler.progress().complete:
                break
        self.assertEqual(assembler.take_completed().payload, data)

    def test_encoder_rejects_empty_and_oversized_payloads(self) -> None:
        with self.assertRaises(StreamError) as caught:
            StreamEncoder(b"", 0)
        self.assertIs(caught.exception.kind, StreamErrorKind.EMPTY_PAYLOAD)
        with self.assertRaises(StreamError) as caught:
            StreamEncoder(bytes(MAX_PAYLOAD_LEN + 1), 0)
        self.assertIs(caught.exception.kind, StreamErrorKind.PAYLOAD_TOO_LARGE)
        with self.assertRaises(ValueError):
            StreamEncoder(b"x", 256)

    def test_beacon_roundtrip_through_lane_d(self) -> None:
        encoder = StreamEncoder(payload(77, 9), 3)
        _, _, d = encoder.lane_data(512)
        beacon = parse_d_lane(d)
        self.assertIsInstance(beacon, Beacon)
        self.assertEqual(beacon.meta, encoder.meta)
        self.assertEqual(beacon.header.frame, 512)
        self.assertIsNone(parse_d_lane(d[:11]))
        bad = bytearray(d)
        bad[3] = 0x20
        self.assertIsNone(parse_d_lane(bytes(bad)))
        zero_length = bytearray(d)
        zero_length[5:8] = b"\x00\x00\x00"
        self.assertIsNone(parse_d_lane(bytes(zero_length)))
        # non-beacon frames carry an atom instead
        _, _, d = encoder.lane_data(513)
        self.assertIsInstance(parse_d_lane(d), AtomPacket)

    def test_atom_lane_parsers_check_lane_and_length(self) -> None:
        encoder = StreamEncoder(payload(500, 8), 1)
        p, k, d = encoder.lane_data(6)
        packet = parse_atom_lane(Lane.K, k)
        self.assertEqual(packet.first_id, first_atom_id(6) + 2)
        self.assertEqual(len(packet.atoms), 5)
        self.assertEqual(parse_atom_lane(Lane.P, p).first_id, first_atom_id(6))
        self.assertEqual(parse_d_lane(d).first_id, first_atom_id(6) + 1)
        self.assertIsNone(parse_atom_lane(Lane.D, d))
        self.assertIsNone(parse_atom_lane(Lane.P, p[:-1]))

    def test_pending_atoms_are_bounded_and_reset_forgets_everything(self) -> None:
        data = payload(3_000, 26)
        encoder = StreamEncoder(data, 1)
        assembler = StreamAssembler(AssemblerLimits(max_pending_atoms=10))
        for frame in (1, 2, 3, 5, 6):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
        self.assertEqual(len(assembler._pending), 10)
        self.assertEqual(assembler.progress().atoms_received, 0)
        feed_frame(assembler, encoder, 4, [Lane.D])
        progress = assembler.progress()
        self.assertEqual(progress.meta, encoder.meta)
        # the ten newest pending atoms were replayed into the decoder
        self.assertEqual(progress.atoms_received, 10)
        self.assertEqual(progress.rank, 10)
        assembler.reset()
        self.assertIsNone(assembler.progress().meta)
        self.assertEqual(assembler.progress().atoms_received, 0)

    def test_atoms_of_another_stream_are_ignored(self) -> None:
        mine = StreamEncoder(payload(200, 27), 1)
        other = StreamEncoder(payload(200, 28), 1)
        self.assertNotEqual(mine.meta.tag, other.meta.tag)
        assembler = StreamAssembler()
        feed_frame(assembler, mine, 0, [Lane.D])
        feed_frame(assembler, other, 1, (Lane.P, Lane.K, Lane.D))
        self.assertEqual(assembler.progress().atoms_received, 0)

    def test_cells_match_the_transmitted_words(self) -> None:
        encoder = StreamEncoder(payload(64, 29), 4)
        p, k, d = encoder.words(3)
        cells = encoder.cells(3)
        self.assertEqual((cells.p_word(), cells.k_word(), cells.d_word()), (p, k, d))
        with self.assertRaises(ValueError):
            encoder.lane_data(70_000)


class StreamSoakTest(unittest.TestCase):
    """Ports of ``crates/iroha_petal/tests/streams.rs``."""

    #: The same 400 trials (same random sequence) as the reference.
    TRIALS = 400

    def test_random_streams_always_complete_and_never_deliver_wrong_data(self) -> None:
        rng = Xorshift32(0xC0FFEE11)
        for trial in range(self.TRIALS):
            # sizes cover K = 1, a few atoms, and a few hundred atoms
            if trial % 8 == 0:
                length = 1 + rng.next_u32() % 16
            elif trial % 8 == 1:
                length = 17 + rng.next_u32() % 100
            else:
                length = 1 + rng.next_u32() % 3_000
            data = bytes(rng.next_byte() for _ in range(length))
            kind = rng.next_byte()
            encoder = StreamEncoder(data, kind)
            loss_percent = rng.next_u32() % 70
            lanes = (
                [Lane.P],
                [Lane.D, Lane.P],
                [Lane.K, Lane.D],
                [Lane.P, Lane.K, Lane.D],
                [Lane.P, Lane.K, Lane.D],
            )[rng.next_u32() % 5]
            assembler = StreamAssembler()
            frame = rng.next_u32() & 0xFFFF
            shown = 0
            budget = 40 + 8 * (length // 13 + 2) * 100 // (100 - loss_percent)
            while not assembler.progress().complete:
                if rng.next_u32() % 100 >= loss_percent:
                    # only lane D carries the beacon, so a receiver must read it at
                    # least once; offer it on beacon frames whatever else is readable
                    readable = list(lanes)
                    if is_beacon_frame(frame) and Lane.D not in readable:
                        readable.append(Lane.D)
                    feed_frame(assembler, encoder, frame, readable)
                frame = (frame + 1) & 0xFFFF
                shown += 1
                self.assertLess(
                    shown,
                    budget,
                    f"trial {trial}: {length} bytes, loss {loss_percent} %, lanes {lanes}",
                )
            done = assembler.take_completed()
            self.assertEqual(done.payload, data, f"trial {trial}")
            self.assertEqual(done.meta.kind, kind)

    def test_counter_wraparound_keeps_atom_ids_consistent(self) -> None:
        data = bytes((i * 7 + 3) & 0xFF for i in range(2_000))
        encoder = StreamEncoder(data, 1)
        assembler = StreamAssembler()
        # start a few frames before the 16-bit counter wraps and run across it
        frame = 65_530
        for _ in range(200):
            feed_frame(assembler, encoder, frame, (Lane.P, Lane.K, Lane.D))
            frame = (frame + 1) & 0xFFFF
            if assembler.progress().complete:
                break
        self.assertEqual(assembler.take_completed().payload, data)


if __name__ == "__main__":
    unittest.main()
