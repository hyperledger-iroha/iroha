package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Ports of the reference `session` tests plus timeout and simulated-camera robustness checks. */
class PetalScanSessionTest {
    private fun payload(length: Int): ByteArray = PetalTestSupport.payload(length, 3)

    private fun render(encoder: PetalStreamEncoder, frame: Int): PetalRgb =
        PetalRenderer.render(encoder.cells(frame), PetalRenderOptions(512, 2))

    private fun camera(rotationDeg: Double): PetalCaptureConfig = PetalCameraSimulator.fitToFrame(
        PetalCaptureConfig.modern().copy(width = 640, height = 480, rotationDeg = rotationDeg),
        4.0,
    )

    @Test
    fun aSteadyCameraIsTrackedAfterTheFirstFrame() {
        val encoder = PetalStreamEncoder(payload(3000), 2)
        val config = camera(8.0)
        val session = PetalScanSession()
        for (frame in 0 until 6) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config), frame * 100L)
            assertNull(outcome.error, "frame $frame")
        }
        assertEquals(6L, session.stats().readable)
        assertEquals(5L, session.stats().tracked, "every frame after the first follows the pose")
        // a pause longer than the tracking window forces a full search again
        val late = 5 * 100L + PetalScanSession.TRACK_WINDOW_MILLIS + 1
        session.push(PetalCameraSimulator.capture(render(encoder, 6), config), late)
        assertEquals(5L, session.stats().tracked)
        assertEquals(7L, session.stats().readable)
    }

    @Test
    fun aSessionReceivesAPayloadFromSimulatedCaptures() {
        val data = payload(5000)
        val encoder = PetalStreamEncoder(data, 2)
        val config = camera(20.0)
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        for (frame in 0 until 40) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config), frame * 100L)
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        val completed = assertNotNull(done, "completed")
        assertContentEquals(data, completed.payload)
        assertEquals(2, completed.meta.kind)
        val stats = session.stats()
        assertTrue(stats.readable > 0 && stats.laneD > 0 && stats.fieldLanes > 0, "$stats")
        // 5,000 bytes need 313 atoms: a clean camera reads them in 15 or 16 frames
        assertTrue(stats.frames <= 17, "$stats")
        assertTrue(session.progress().complete)
    }

    @Test
    fun idleSessionsForgetPartialStreams() {
        val encoder = PetalStreamEncoder(payload(12_000), 1)
        val session = PetalScanSession(PetalScanLimits(idleTimeoutMillis = 1_000))
        session.push(PetalCameraSimulator.capture(render(encoder, 0), camera(0.0)), 0)
        assertTrue(session.progress().rank > 0)
        // a frame much later with nothing readable resets the session first
        val outcome = session.push(PetalLuma(640, 480), 60_000)
        assertEquals(PetalDecodeError.NO_FINDERS, outcome.error)
        assertEquals(0, outcome.progress.rank)
        assertNull(outcome.progress.meta)
    }

    @Test
    fun locatedCountsCodesThatWereSeenButCouldNotBeRead() {
        val encoder = PetalStreamEncoder(payload(100), 1)
        val data = render(encoder, 1).data()
        // keep only the four blossoms: finders are located, no lane can be read
        val scale = 512.0 / 1024.0
        for (y in 0 until 512) {
            for (x in 0 until 512) {
                val nearFinder = (0 until 4).any { finder ->
                    val dx = x - PetalLayout.finderCenterX(finder) * scale
                    val dy = y - PetalLayout.finderCenterY(finder) * scale
                    Math.sqrt(dx * dx + dy * dy) < 34.0
                }
                if (!nearFinder) for (c in 0 until 3) data[(y * 512 + x) * 3 + c] = 0
            }
        }
        val finders = assertNotNull(PetalRgb.fromRaw(512, 512, data)).toLuma()
        val session = PetalScanSession()
        val outcome = session.push(finders, 0)
        assertEquals(PetalDecodeError.NO_ORIENTATION, outcome.error)
        assertEquals(1L, session.stats().located)
        assertEquals(0L, session.stats().readable)
        // a frame with no code at all is not "located"
        session.push(PetalLuma(320, 240), 100)
        assertEquals(1L, session.stats().located)
        assertEquals(2L, session.stats().frames)
    }

    @Test
    fun unreadableFramesDoNotDisturbProgress() {
        val session = PetalScanSession()
        val outcome = session.push(PetalLuma(320, 240), 5)
        assertNull(outcome.completed)
        assertTrue(outcome.lanes.isEmpty())
        assertEquals(1L, session.stats().frames)
        assertEquals(0L, session.stats().located)
        assertFailsWith<IllegalArgumentException> { session.push(PetalLuma(320, 240), -1) }
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, session.push(PetalLuma(10, 10), 6).error)
    }

    /** The `one-pass` stream of the golden captures. */
    private fun onePass(): PetalStreamEncoder = PetalStreamEncoder(PetalTestSupport.payload(3000, 2), 2)

    @Test
    fun mergingRecoversFieldLanesThatNoSingleCaptureReads() {
        val encoder = onePass()
        val shown = render(encoder, 5)
        // a soft 480p camera in very dim light: every capture loses its field lanes to sensor
        // noise, which differs from capture to capture
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.modern().copy(
                width = 640, height = 480, fill = 0.85, rotationDeg = 31.0, tiltXDeg = -7.0, blurSigma = 1.4, noise = 50.0,
            ),
            4.0,
        )
        val session = PetalScanSession()
        val merged = StringBuilder()
        var singles = 0
        for ((index, seed) in (1201L..1204L).withIndex()) {
            val image = PetalCameraSimulator.capture(shown, config.copy(seed = seed))
            val single = assertNotNull(PetalDecoder.decode(image).frame, "lane D reads")
            singles += single.fields.count { it != null }
            merged.append(session.push(image, 33L * index).merged)
        }
        assertTrue(singles <= 1, "the captures read $singles field lanes alone")
        assertTrue(merged.length >= 3, "merging recovered $merged")
        val stats = session.stats()
        assertEquals(merged.length.toLong(), stats.mergedLanes)
        assertTrue(stats.merges >= 1)
        // the merged atoms wait for the beacon, whose lane D the same camera reads
        assertEquals(0L, session.progress().atomsReceived)
        val beacon = PetalCameraSimulator.capture(render(encoder, 4), config.copy(seed = 1205))
        session.push(beacon, 200)
        assertEquals(encoder.meta, session.progress().meta)
        assertTrue(session.progress().atomsReceived >= 5L * merged.length)
    }

    /**
     * A clean capture of frame [petals] whose decoded lanes are replaced by lane `D` of frame
     * [rings], as if the rings showed another frame than the petal field.
     */
    private fun ringsOfAnotherFrame(encoder: PetalStreamEncoder, petals: Int, rings: Int, seed: Long): Pair<PetalLuma, PetalDecodedFrame> {
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.modern().copy(width = 640, height = 480, rotationDeg = 25.0, seed = seed),
            4.0,
        )
        val image = PetalCameraSimulator.capture(render(encoder, petals), config)
        val frame = assertNotNull(PetalDecoder.decode(image).frame, "decodes")
        assertTrue(frame.fields.all { it != null })
        val replaced = PetalDecodedFrame(
            frame.pose,
            frame.rotation,
            frame.mirrored,
            PetalLaneResult(encoder.laneData(rings).d(), 0, 0),
            listOf(null, null, null, null),
            frame.inferredCorner,
        )
        return image to replaced
    }

    @Test
    fun aMergeNeverOffersAtomsOfAnotherShownFrame() {
        val encoder = onePass()
        val session = PetalScanSession()
        for (seed in 1L..3L) {
            val (image, frame) = ringsOfAnotherFrame(encoder, 11, 10, seed)
            assertEquals("", session.merge(image, frame), "capture $seed")
        }
        val stats = session.stats()
        assertEquals(2L, stats.merges)
        assertEquals(0L, stats.mergedLanes)
        assertEquals(0L, session.progress().atomsReceived)
        // what was refused: the merged field lanes name frame 11, not frame 10
        val held = session.held.first
        assertEquals(10, held.shown.frame)
        val merged = PetalDecoder.mergeFields(held.evidence, BooleanArray(PetalLayout.SECTORS), PetalWorkspace())
        val truth = encoder.laneData(11)
        for (sector in 0 until PetalLayout.SECTORS) assertContentEquals(truth.field(sector), merged[sector]?.data, "sector $sector")
    }

    @Test
    fun tornCapturesWhoseLanesNameDifferentFramesAreNotHeld() {
        val encoder = onePass()
        val session = PetalScanSession()
        val (image, frame) = ringsOfAnotherFrame(encoder, 11, 10, 1)
        val torn = PetalDecodedFrame(
            frame.pose,
            frame.rotation,
            frame.mirrored,
            frame.d,
            listOf(null, null, PetalLaneResult(encoder.laneData(11).field(2), 0, 0), null),
            frame.inferredCorner,
        )
        assertEquals("", session.merge(image, torn))
        assertTrue(session.held.isEmpty())
    }

    @Test
    fun theSessionHoldsAtMostFourCapturesOfSixShownFrames() {
        val encoder = onePass()
        val session = PetalScanSession()
        // refused merges never finish a shown frame, so its captures stay held
        for (seed in 1L..PetalScanSession.MERGE_CAPTURES + 2L) {
            val (image, frame) = ringsOfAnotherFrame(encoder, 11, 10, seed)
            session.merge(image, frame)
        }
        assertEquals(1, session.held.size)
        assertEquals(PetalScanSession.MERGE_CAPTURES, session.held.first.evidence.size)
        assertEquals(PetalScanSession.MERGE_CAPTURES + 1L, session.stats().merges)
        val (image, _) = ringsOfAnotherFrame(encoder, 11, 10, 1)
        for (rings in 12 until 12 + PetalScanSession.MERGE_WINDOW) {
            val (_, frame) = ringsOfAnotherFrame(encoder, 11, rings, 1)
            session.merge(image, frame)
        }
        assertEquals(PetalScanSession.MERGE_WINDOW, session.held.size)
        assertTrue(session.held.all { it.shown.frame >= 12 })
        session.reset()
        assertTrue(session.held.isEmpty())
    }

    private fun withoutFields(frame: PetalDecodedFrame) =
        PetalDecodedFrame(frame.pose, frame.rotation, frame.mirrored, frame.d, listOf(null, null, null, null), frame.inferredCorner)

    @Test
    fun switchingStreamsWithTheSameShortTagDiscardsOldMergeState() {
        fun encoder(last: Int): PetalStreamEncoder {
            val payload = ByteArray(100)
            payload[3] = last.toByte()
            return PetalStreamEncoder(payload, 2)
        }
        val old = encoder(49)
        val new = encoder(128)
        assertEquals(61, old.meta.tag)
        assertEquals(61, new.meta.tag)
        assertTrue(old.meta != new.meta)
        val oldImage = render(old, 0).toLuma()
        val newImage = render(new, 0).toLuma()
        val oldFull = assertNotNull(PetalDecoder.decode(oldImage).frame)
        val newPartial = withoutFields(assertNotNull(PetalDecoder.decode(newImage).frame))
        for (wasComplete in listOf(false, true)) {
            val session = PetalScanSession()
            val previous = if (wasComplete) oldFull else withoutFields(oldFull)
            session.absorb(previous)
            session.merge(oldImage, previous)
            assertTrue(session.held.isNotEmpty())
            session.absorb(newPartial)
            session.absorb(newPartial)
            assertEquals(new.meta, session.progress().meta)
            assertTrue(session.held.isEmpty())
            assertEquals("", session.merge(newImage, newPartial))
            assertEquals(1, session.held.first.evidence.size)
            assertTrue(session.held.first.done.none { it })
            assertEquals("0123", session.merge(newImage, newPartial))
        }
    }

    @Test
    fun theFirstBeaconPreservesEvidenceReceivedBeforeIt() {
        val encoder = onePass()
        val image = render(encoder, 1).toLuma()
        val partial = withoutFields(assertNotNull(PetalDecoder.decode(image).frame))
        val session = PetalScanSession()
        session.absorb(partial)
        session.merge(image, partial)
        assertNull(session.progress().meta)
        assertEquals(1, session.held.first.evidence.size)
        val beacon = withoutFields(assertNotNull(PetalDecoder.decode(render(encoder, 0).toLuma()).frame))
        session.absorb(beacon)
        assertEquals(encoder.meta, session.progress().meta)
        assertEquals(1, session.held.first.evidence.size)
        assertEquals("0123", session.merge(image, partial))
    }

    /** Frame [frame] rendered at 512 pixels with the blossom of canonical corner [corner] painted over. */
    private fun hiddenBlossom(encoder: PetalStreamEncoder, frame: Int, corner: Int): PetalLuma {
        val rgb = render(encoder, frame)
        val n = rgb.width
        val scale = n / 1024.0
        val reach = 75.0 * scale
        val data = rgb.data()
        for (y in 0 until n) {
            for (x in 0 until n) {
                val dx = x + 0.5 - PetalLayout.finderCenterX(corner) * scale
                val dy = y + 0.5 - PetalLayout.finderCenterY(corner) * scale
                if (dx * dx + dy * dy <= reach * reach) for (c in 0 until 3) data[(y * n + x) * 3 + c] = 0
            }
        }
        return assertNotNull(PetalRgb.fromRaw(n, n, data)).toLuma()
    }

    @Test
    fun inferredCornersAreCountedAndResetForgetsThePose() {
        val encoder = PetalStreamEncoder(payload(2000), 1)
        val session = PetalScanSession()
        assertEquals("D0123", session.push(render(encoder, 0).toLuma(), 0).lanes)
        // a thumb arrives over the bottom-right blossom: tracking infers it
        assertEquals("D0123", session.push(hiddenBlossom(encoder, 1, 2), 125).lanes)
        assertEquals(1L, session.stats().tracked)
        assertEquals(1L, session.stats().inferred)
        // a full search infers it as well
        session.reset()
        assertEquals("D0123", session.push(hiddenBlossom(encoder, 2, 2), 250).lanes)
        assertEquals(1L, session.stats().tracked, "reset forgets the pose")
        assertEquals(2L, session.stats().inferred)
        assertEquals(3L, session.stats().readable)
    }

    @Test
    fun legacyCapturesStillDeliverThePayload() {
        val data = payload(3000)
        val encoder = PetalStreamEncoder(data, 1)
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.legacy().copy(width = 960, height = 540, rotationDeg = 200.0, seed = 12),
            4.0,
        )
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        for (frame in 0 until 40) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config.copy(seed = frame + 1L)), frame * 125L)
            assertTrue(outcome.lanes.all { it in "D0123" })
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        assertContentEquals(data, assertNotNull(done, "completed after ${session.stats()}").payload)
    }

    @Test
    fun overExposedCapturesStillReadEveryLane() {
        val data = payload(3000)
        val encoder = PetalStreamEncoder(data, 1)
        // an auto-exposure that blows a mostly black screen out three-fold, seen at an angle
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.modern().copy(
                width = 960, height = 540, rotationDeg = 28.0, tiltXDeg = 12.0, tiltYDeg = -10.0, exposure = 3.0,
            ),
            4.0,
        )
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        var pushed = 0
        for (frame in 0 until 12) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config.copy(seed = frame + 1L)), frame * 125L)
            pushed += 1
            assertEquals("D0123", outcome.lanes, "frame $frame")
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        assertContentEquals(data, assertNotNull(done, "completed after ${session.stats()}").payload)
        // 3,000 bytes are 188 atoms: ten frames of every lane
        assertTrue(pushed <= 10, "$pushed frames with every lane")
        assertEquals(4L * pushed, session.stats().fieldLanes)
    }

    @Test
    fun absoluteTimeoutForgetsStreamsThatNeverFinish() {
        val encoder = PetalStreamEncoder(payload(4000), 1)
        val session = PetalScanSession(PetalScanLimits(idleTimeoutMillis = 10_000, absoluteTimeoutMillis = 2_000))
        val first = session.push(render(encoder, 0).toLuma(), 0)
        assertEquals("D0123", first.lanes)
        assertTrue(session.push(render(encoder, 1).toLuma(), 1_500).progress.rank > first.progress.rank)
        // still progressing, but the stream started more than 2 s ago
        val late = session.push(render(encoder, 2).toLuma(), 2_600)
        assertEquals(null, late.error)
        assertTrue(late.progress.rank <= PetalStream.atomsInFrame(2), "rank restarted: ${late.progress.rank}")
        assertNull(late.progress.meta, "frame 2 carries no beacon, so the stream identity is forgotten")
    }

    @Test
    fun cleanFramesCountLanesAndDeliverExactlyOnce() {
        val data = payload(2000)
        val encoder = PetalStreamEncoder(data, 7)
        val session = PetalScanSession()
        var deliveries = 0
        for (frame in 0 until encoder.systematicFrames + 2) {
            val outcome = session.push(render(encoder, frame).toLuma(), frame * 100L)
            outcome.completed?.let {
                deliveries += 1
                assertContentEquals(data, it.payload)
            }
        }
        assertEquals(1, deliveries)
        val stats = session.stats()
        assertEquals(stats.frames, stats.located)
        assertEquals(stats.frames, stats.laneD)
        assertEquals(4 * stats.frames, stats.fieldLanes)
        assertEquals(0L, stats.merges)
        session.reset()
        assertNull(session.progress().meta)
    }
}
