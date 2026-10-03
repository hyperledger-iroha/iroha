package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.feedFrame
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.payload

/** Ports of the reference `fountain` and `stream` unit tests plus loss, reorder and limit tests. */
class PetalFountainStreamTest {
    private val allLanes = listOf(PetalLane.D, PetalLane.P, PetalLane.K)

    private fun reassemble(atoms: List<ByteArray>, length: Int): ByteArray {
        val bytes = ByteArray(atoms.size * PetalLanes.ATOM_LEN)
        atoms.forEachIndexed { index, atom -> atom.copyInto(bytes, index * PetalLanes.ATOM_LEN) }
        return bytes.copyOf(length)
    }

    @Test
    fun systematicAtomsAloneRecoverThePayload() {
        val data = payload(100, 5)
        val source = PetalFountain.splitPayload(data)
        val decoder = PetalFountainDecoder(source.size)
        source.forEachIndexed { id, atom -> assertTrue(decoder.addEncoded(7, id, atom)) }
        assertContentEquals(data, reassemble(assertNotNull(decoder.solve()), 100))
        assertContentEquals(source[3], PetalFountain.encodeAtom(source, 7, 3))
    }

    @Test
    fun repairAtomsCoverForLostSystematicAtoms() {
        val data = payload(1000, 9)
        val source = PetalFountain.splitPayload(data)
        val k = source.size
        val crc = 0x1234_5678
        val decoder = PetalFountainDecoder(k)
        // lose every third systematic atom, then take repair atoms
        for (id in 0 until k) {
            if (id % 3 != 0) decoder.addEncoded(crc, id, PetalFountain.encodeAtom(source, crc, id))
        }
        var id = k
        var used = 0
        while (!decoder.isComplete) {
            decoder.addEncoded(crc, id, PetalFountain.encodeAtom(source, crc, id))
            id += 1
            used += 1
            assertTrue(used < k, "decoder must converge")
        }
        val missing = (0 until k).count { it % 3 == 0 }
        assertTrue(used <= missing + 8, "needed $used repairs for $missing missing")
        assertContentEquals(data, reassemble(assertNotNull(decoder.solve()), 1000))
    }

    @Test
    fun pureRepairStreamsDecodeWithSmallOverhead() {
        val data = payload(5000, 11)
        val source = PetalFountain.splitPayload(data)
        val k = source.size
        val crc = 0xCAFE_F00D.toInt()
        var totalOverhead = 0
        for (trial in 0 until 20) {
            val decoder = PetalFountainDecoder(k)
            var id = k + trial * 1000
            var received = 0
            while (!decoder.isComplete) {
                decoder.addEncoded(crc, id, PetalFountain.encodeAtom(source, crc, id))
                id += 1
                received += 1
            }
            totalOverhead += received - k
            assertContentEquals(data, reassemble(assertNotNull(decoder.solve()), 5000))
        }
        assertTrue(totalOverhead <= 20 * 4, "average overhead ${totalOverhead / 20.0}")
    }

    @Test
    fun duplicateAndDependentAtomsDoNotRaiseRank() {
        val source = PetalFountain.splitPayload(payload(60, 3))
        val decoder = PetalFountainDecoder(source.size)
        assertTrue(decoder.addEncoded(1, 0, source[0]))
        assertFalse(decoder.addEncoded(1, 0, source[0]))
        assertEquals(1, decoder.rank)
        assertNull(decoder.solve())
        assertFalse(decoder.add(IntArray(2), ByteArray(PetalLanes.ATOM_LEN)), "wrong mask length")
    }

    @Test
    fun repairMasksSpanFarMoreThanThirtyTwoDimensions() {
        // Regression: an xorshift-derived mask is GF(2)-linear in a 32-bit seed and can never exceed rank 32.
        val k = 200
        val decoder = PetalFountainDecoder(k)
        for (id in k until k + 400) decoder.add(PetalFountain.maskWords(k, 5, id), ByteArray(PetalLanes.ATOM_LEN))
        assertEquals(k, decoder.rank, "repair masks must reach full rank")
    }

    @Test
    fun masksAreNonzeroAndPaddedBitsAreClear() {
        for (k in intArrayOf(1, 2, 31, 32, 33, 100)) {
            for (id in 0 until 200) {
                val mask = PetalFountain.maskWords(k, 99, id)
                assertTrue(mask.any { it != 0 })
                if (k % 32 != 0) assertEquals(0, mask[mask.size - 1] ushr (k % 32))
            }
        }
        assertFailsWith<IllegalArgumentException> { PetalFountain.maskWords(0, 1, 1) }
    }

    @Test
    fun lossyReorderedRepairStreamsStillRecover() {
        val data = payload(3_000, 17)
        val source = PetalFountain.splitPayload(data)
        val k = source.size
        val crc = PetalCrc32c.compute(data)
        val ids = (0 until 3 * k).toMutableList()
        // deterministic shuffle, then drop 40 % of the atoms
        val rng = PetalXorshift32(4242)
        for (i in ids.size - 1 downTo 1) {
            val j = ((rng.nextInt().toLong() and 0xFFFF_FFFFL) % (i + 1)).toInt()
            val swap = ids[i]
            ids[i] = ids[j]
            ids[j] = swap
        }
        val decoder = PetalFountainDecoder(k)
        var offered = 0
        for (id in ids) {
            if ((rng.nextInt() ushr 8) % 10 < 4) continue
            decoder.addEncoded(crc, id, PetalFountain.encodeAtom(source, crc, id))
            offered += 1
            if (decoder.isComplete) break
        }
        assertTrue(decoder.isComplete, "offered $offered atoms")
        assertContentEquals(data, reassemble(assertNotNull(decoder.solve()), data.size))
    }

    @Test
    fun atomIdsAreContiguousAcrossFrames() {
        var expected = 0
        for (frame in 0..70) {
            assertEquals(expected, PetalStream.firstAtomId(frame), "frame $frame")
            expected += PetalStream.atomsInFrame(frame)
        }
        assertEquals(6, PetalStream.atomsInFrame(0))
        assertEquals(7, PetalStream.atomsInFrame(1))
        assertEquals(PetalStream.firstAtomId(4) + 1, PetalStream.laneFirstId(PetalLane.K, 4))
        assertEquals(PetalStream.firstAtomId(5) + 2, PetalStream.laneFirstId(PetalLane.K, 5))
        // the frame counter wraps on a beacon frame, so ids repeat cleanly
        assertTrue(PetalStream.isBeaconFrame(0) && 65_536 % PetalStream.BEACON_INTERVAL == 0)
        assertFailsWith<IllegalArgumentException> { PetalStream.firstAtomId(65_536) }
        assertFailsWith<IllegalArgumentException> { PetalStream.isBeaconFrame(-1) }
    }

    @Test
    fun cleanStreamCompletesAfterOneSystematicPass() {
        val data = payload(1_000, 21)
        val encoder = PetalStreamEncoder(data, 2)
        val assembler = PetalStreamAssembler()
        for (frame in 0 until encoder.systematicFrames) feedFrame(assembler, encoder, frame, allLanes)
        val done = assertNotNull(assembler.takeCompleted())
        assertContentEquals(data, done.payload)
        assertEquals(2, done.meta.kind)
        assertTrue(assembler.progress().complete)
        assertNull(assembler.takeCompleted(), "a payload is delivered exactly once")
    }

    @Test
    fun anySingleLaneIsEnoughGivenABeacon() {
        val data = payload(400, 22)
        val encoder = PetalStreamEncoder(data, 1)
        for (lane in listOf(PetalLane.P, PetalLane.K, PetalLane.D)) {
            val assembler = PetalStreamAssembler()
            feedFrame(assembler, encoder, 0, listOf(PetalLane.D))
            for (frame in 0 until 400) {
                feedFrame(assembler, encoder, frame, listOf(lane))
                if (assembler.progress().complete) break
            }
            assertContentEquals(data, assertNotNull(assembler.takeCompleted(), "$lane alone").payload)
        }
    }

    @Test
    fun atomsSeenBeforeTheFirstBeaconAreNotLost() {
        val data = payload(300, 23)
        val encoder = PetalStreamEncoder(data, 1)
        val assembler = PetalStreamAssembler()
        // frames 1..3: no beacon among them until frame 4
        for (frame in 1 until 4) feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
        assertNull(assembler.progress().meta)
        for (frame in 4 until encoder.systematicFrames + 4) {
            feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
        }
        assertContentEquals(data, assertNotNull(assembler.takeCompleted()).payload)
    }

    @Test
    fun joiningMidStreamAndLosingFramesStillCompletes() {
        val data = payload(2_500, 24)
        val encoder = PetalStreamEncoder(data, 3)
        val assembler = PetalStreamAssembler()
        val rng = PetalXorshift32(77)
        var frame = 15 // join late
        var shown = 0
        while (!assembler.progress().complete) {
            // 60 % of the frames are readable
            if ((rng.nextInt().toLong() and 0xFFFF_FFFFL) % 10 < 6) feedFrame(assembler, encoder, frame, allLanes)
            frame = (frame + 1) and PetalStream.MAX_FRAME
            shown += 1
            assertTrue(shown < 600, "stream failed to complete")
        }
        assertContentEquals(data, assertNotNull(assembler.takeCompleted()).payload)
    }

    @Test
    fun aDifferentStreamReplacesTheActiveOneAfterTwoBeacons() {
        val first = PetalStreamEncoder(payload(100, 1), 1)
        val second = PetalStreamEncoder(payload(100, 2), 1)
        val assembler = PetalStreamAssembler()
        feedFrame(assembler, first, 0, listOf(PetalLane.D))
        assertEquals(first.meta, assembler.progress().meta)
        feedFrame(assembler, second, 0, listOf(PetalLane.D))
        assertEquals(first.meta, assembler.progress().meta)
        feedFrame(assembler, second, 4, listOf(PetalLane.D))
        assertEquals(second.meta, assembler.progress().meta)
    }

    @Test
    fun anInterruptedConflictDoesNotSwitchStreams() {
        val first = PetalStreamEncoder(payload(100, 1), 1)
        val second = PetalStreamEncoder(payload(100, 2), 1)
        val assembler = PetalStreamAssembler()
        feedFrame(assembler, first, 0, listOf(PetalLane.D))
        feedFrame(assembler, second, 0, listOf(PetalLane.D))
        feedFrame(assembler, first, 4, listOf(PetalLane.D)) // the active stream clears the conflict
        feedFrame(assembler, second, 8, listOf(PetalLane.D))
        assertEquals(first.meta, assembler.progress().meta)
    }

    @Test
    fun oversizedBeaconsAreIgnored() {
        val encoder = PetalStreamEncoder(payload(4_000, 5), 1)
        val assembler = PetalStreamAssembler(PetalAssemblerLimits(maxPayloadLength = 1_000))
        feedFrame(assembler, encoder, 0, listOf(PetalLane.D))
        assertNull(assembler.progress().meta)
    }

    @Test
    fun pendingAtomsAreBoundedAndOldestAreEvicted() {
        val data = payload(64, 31) // 4 source atoms
        val encoder = PetalStreamEncoder(data, 1)
        val assembler = PetalStreamAssembler(PetalAssemblerLimits(maxPendingAtoms = 2))
        // lane K of frame 1 carries repair atoms 8..12, seen before any beacon
        feedFrame(assembler, encoder, 1, listOf(PetalLane.K))
        assertNull(assembler.progress().meta)
        feedFrame(assembler, encoder, 0, listOf(PetalLane.D))
        // only the two newest pending atoms were replayed
        assertEquals(2L, assembler.progress().atomsReceived)
    }

    @Test
    fun aZeroPendingLimitBuffersNothing() {
        val encoder = PetalStreamEncoder(payload(300, 41), 1)
        val assembler = PetalStreamAssembler(PetalAssemblerLimits(maxPendingAtoms = 0))
        // atoms before any beacon are dropped, not buffered
        for (frame in 1 until 4) feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
        feedFrame(assembler, encoder, 4, listOf(PetalLane.D))
        assertEquals(0, assembler.progress().rank)
        assertEquals(0L, assembler.progress().atomsReceived)
        assertEquals(encoder.meta, assembler.progress().meta)
    }

    @Test
    fun atomsOfAnotherStreamAreIgnoredOnceAStreamIsActive() {
        val first = PetalStreamEncoder(payload(200, 41), 1)
        val other = PetalStreamEncoder(payload(200, 42), 1)
        assertTrue(first.meta.tag != other.meta.tag)
        val assembler = PetalStreamAssembler()
        feedFrame(assembler, first, 0, listOf(PetalLane.D))
        feedFrame(assembler, other, 1, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
        assertEquals(0L, assembler.progress().atomsReceived)
        assertEquals(0, assembler.progress().rank)
    }

    @Test
    fun resetForgetsTheStreamButKeepsIntegrityFailures() {
        val encoder = PetalStreamEncoder(payload(40, 43), 1) // 3 source atoms
        val assembler = PetalStreamAssembler()
        feedFrame(assembler, encoder, 0, listOf(PetalLane.D))
        val wrong = ByteArray(PetalLanes.ATOM_LEN) { 0x11 }
        assembler.pushAtoms(PetalAtomPacket(PetalLaneHeader(encoder.meta.tag, 0), 0, listOf(wrong, wrong, wrong)))
        assertEquals(1L, assembler.progress().integrityFailures)
        feedFrame(assembler, encoder, 1, allLanes)
        assertTrue(assembler.progress().rank > 0)
        assembler.reset()
        assertEquals(PetalProgress(null, 0, 0, 0, 1, false), assembler.progress())
    }

    @Test
    fun corruptAtomsAreCaughtByThePayloadCrc() {
        val data = payload(200, 25)
        val encoder = PetalStreamEncoder(data, 1)
        val assembler = PetalStreamAssembler()
        feedFrame(assembler, encoder, 0, listOf(PetalLane.D, PetalLane.K))
        // atom 0 arrives with a valid header but a wrong body
        assembler.pushAtoms(
            PetalAtomPacket(PetalLaneHeader(encoder.meta.tag, 0), 0, listOf(ByteArray(PetalLanes.ATOM_LEN) { 0xEE.toByte() })),
        )
        for (frame in 1 until encoder.systematicFrames) {
            feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
        }
        assertNull(assembler.takeCompleted())
        assertEquals(1L, assembler.progress().integrityFailures)
        // clean repair frames after the reset recover the payload
        for (frame in 100 until 600) {
            feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
            if (assembler.progress().complete) break
        }
        assertContentEquals(data, assertNotNull(assembler.takeCompleted(), "recovered").payload)
    }

    @Test
    fun randomStreamsAlwaysCompleteAndNeverDeliverWrongData() {
        // Randomised soak (reference `tests/streams.rs`): arbitrary sizes, join points, loss and lane subsets.
        val rng = PetalXorshift32(0xC0FF_EE11.toInt())
        fun next(): Long = PetalTestSupport.u32(rng.nextInt())
        for (trial in 0 until 400) {
            // sizes cover K = 1, a few atoms, and a few hundred atoms
            val length = when (trial % 8) {
                0 -> 1 + (next() % 16).toInt()
                1 -> 17 + (next() % 100).toInt()
                else -> 1 + (next() % 3_000).toInt()
            }
            val data = ByteArray(length) { rng.nextByte().toByte() }
            val kind = rng.nextByte()
            val encoder = PetalStreamEncoder(data, kind)
            val lossPercent = (next() % 70).toInt()
            val lanes = when ((next() % 5).toInt()) {
                0 -> listOf(PetalLane.P)
                1 -> listOf(PetalLane.D, PetalLane.P)
                2 -> listOf(PetalLane.K, PetalLane.D)
                else -> listOf(PetalLane.P, PetalLane.K, PetalLane.D)
            }
            val assembler = PetalStreamAssembler()
            var frame = (next() and 0xFFFF).toInt()
            var shown = 0
            while (!assembler.progress().complete) {
                if (next() % 100 >= lossPercent) {
                    // only lane D carries the beacon, so a receiver must read it at least
                    // once; offer it on beacon frames whatever else is readable
                    val readable = lanes.toMutableList()
                    if (PetalStream.isBeaconFrame(frame) && PetalLane.D !in readable) readable += PetalLane.D
                    feedFrame(assembler, encoder, frame, readable)
                }
                frame = (frame + 1) and PetalStream.MAX_FRAME
                shown += 1
                val budget = 40 + 8 * (length / 13 + 2) * 100 / (100 - lossPercent)
                assertTrue(
                    shown < budget,
                    "trial $trial: $length bytes, loss $lossPercent %, lanes $lanes exceeded $budget frames",
                )
            }
            val done = assertNotNull(assembler.takeCompleted())
            assertContentEquals(data, done.payload, "trial $trial")
            assertEquals(kind, done.meta.kind)
        }
    }

    @Test
    fun counterWraparoundKeepsAtomIdsConsistent() {
        val data = ByteArray(2_000) { (it * 7 + 3).toByte() }
        val encoder = PetalStreamEncoder(data, 1)
        val assembler = PetalStreamAssembler()
        // start a few frames before the 16-bit counter wraps and run across it
        var frame = 65_530
        repeat(200) {
            if (!assembler.progress().complete) {
                feedFrame(assembler, encoder, frame, listOf(PetalLane.P, PetalLane.K, PetalLane.D))
                frame = (frame + 1) and PetalStream.MAX_FRAME
            }
        }
        assertContentEquals(data, assertNotNull(assembler.takeCompleted()).payload)
    }

    @Test
    fun encoderRejectsEmptyAndOversizedPayloads() {
        assertEquals(
            "petal stream payload is empty",
            assertFailsWith<IllegalArgumentException> { PetalStreamEncoder(ByteArray(0), 0) }.message,
        )
        assertEquals(
            "petal stream payload exceeds the 24-bit length field",
            assertFailsWith<IllegalArgumentException> { PetalStreamEncoder(ByteArray(PetalStream.MAX_PAYLOAD_LEN + 1), 0) }.message,
        )
        assertFailsWith<IllegalArgumentException> { PetalStreamEncoder(ByteArray(1), 256) }
    }

    @Test
    fun beaconRoundtripThroughLaneD() {
        val encoder = PetalStreamEncoder(payload(77, 9), 3)
        val d = encoder.laneData(512).d()
        val lane = assertNotNull(PetalStream.parseDLane(d))
        val beacon = assertNotNull(lane.beacon)
        assertEquals(encoder.meta, beacon.meta)
        assertEquals(512, beacon.header.frame)
        assertNull(PetalStream.parseDLane(d.copyOf(11)))
        val bad = d.copyOf()
        bad[3] = 0x20
        assertNull(PetalStream.parseDLane(bad))
        // non-beacon frames carry an atom instead
        val atoms = assertNotNull(PetalStream.parseDLane(encoder.laneData(513).d())).atoms
        assertNotNull(atoms)
        assertEquals(PetalStream.laneFirstId(PetalLane.D, 513), atoms.firstId)
        assertNull(PetalStream.parseAtomLane(PetalLane.D, d))
        assertNull(PetalStream.parseAtomLane(PetalLane.P, ByteArray(18)))
    }
}
