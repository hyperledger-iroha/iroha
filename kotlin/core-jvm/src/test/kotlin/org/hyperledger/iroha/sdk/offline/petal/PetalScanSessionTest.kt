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

    @Test
    fun aSessionReceivesAPayloadFromSimulatedCaptures() {
        val data = payload(500)
        val encoder = PetalStreamEncoder(data, 2)
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.modern().copy(width = 640, height = 480, rotationDeg = 20.0),
            4.0,
        )
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        for (frame in 0 until 40) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config), frame * 125L)
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        val completed = assertNotNull(done, "completed")
        assertContentEquals(data, completed.payload)
        assertEquals(2, completed.meta.kind)
        assertTrue(session.stats().readable > 0 && session.stats().laneD > 0)
        assertTrue(session.progress().complete)
    }

    @Test
    fun legacyCapturesStillDeliverThePayload() {
        val data = payload(300)
        val encoder = PetalStreamEncoder(data, 1)
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.legacy().copy(width = 960, height = 540, rotationDeg = 200.0, seed = 12),
            4.0,
        )
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        for (frame in 0 until 30) {
            val outcome = session.push(PetalCameraSimulator.capture(render(encoder, frame), config.copy(seed = frame + 1L)), frame * 125L)
            assertTrue(outcome.lanes.all { it in "PKD" })
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        assertContentEquals(data, assertNotNull(done, "completed after ${session.stats()}").payload)
    }

    @Test
    fun overExposedCapturesAreReadThroughTheNormalisedRead() {
        val data = payload(300)
        val encoder = PetalStreamEncoder(data, 1)
        // an auto-exposure that blows a mostly black screen out three-fold, seen at an angle
        val config = PetalCameraSimulator.fitToFrame(
            PetalCaptureConfig.modern().copy(
                width = 960, height = 540, rotationDeg = 28.0, tiltXDeg = 12.0, tiltYDeg = -10.0, exposure = 3.0,
            ),
            4.0,
        )
        val first = PetalCameraSimulator.capture(render(encoder, 1), config)
        // the finder levels cannot describe the blown-out tiles: the level read alone reads neither tile lane
        val pose = assertNotNull(PetalDecoder.decode(first).frame)
        val h = pose.homography.m
        val reference = assertNotNull(PetalDecoder.referenceLevels(first, h), "reference levels")
        val workspace = PetalWorkspace()
        PetalDecoder.samplePatches(first, h, workspace)
        val words = PetalDecoder.tileWords(PetalDecoder.readTiles(reference, PetalDecodeOptions.DEFAULT, workspace))
        assertNull(PetalDecoder.decodeWithErasures(PetalLane.P, words.p, words.pConfidence), "level read, lane P")
        assertNull(PetalDecoder.decodeWithErasures(PetalLane.K, words.k, words.kConfidence), "level read, lane K")
        // the session still reads lanes P, K and D of every frame and finishes within a few frames
        val session = PetalScanSession()
        var done: PetalCompleted? = null
        var pushed = 0
        for (frame in 0 until 8) {
            val luma = PetalCameraSimulator.capture(render(encoder, frame), config.copy(seed = frame + 1L))
            val outcome = session.push(luma, frame * 125L)
            pushed += 1
            assertEquals("PKD", outcome.lanes, "frame $frame")
            if (outcome.completed != null) {
                done = outcome.completed
                break
            }
        }
        assertContentEquals(data, assertNotNull(done, "completed after ${session.stats()}").payload)
        assertTrue(pushed <= 5, "$pushed frames with all three lanes")
        assertEquals(pushed.toLong(), session.stats().laneK)
    }

    @Test
    fun idleSessionsForgetPartialStreams() {
        val encoder = PetalStreamEncoder(payload(4000), 1)
        val config = PetalCameraSimulator.fitToFrame(PetalCaptureConfig.modern().copy(width = 640, height = 480), 4.0)
        val session = PetalScanSession(PetalScanLimits(idleTimeoutMillis = 1_000))
        session.push(PetalCameraSimulator.capture(render(encoder, 0), config), 0)
        assertTrue(session.progress().rank > 0)
        // a frame much later with nothing readable resets the session first
        val outcome = session.push(PetalLuma(640, 480), 60_000)
        assertEquals(PetalDecodeError.NO_FINDERS, outcome.error)
        assertEquals(0, outcome.progress.rank)
        assertNull(outcome.progress.meta)
    }

    @Test
    fun absoluteTimeoutForgetsStreamsThatNeverFinish() {
        val encoder = PetalStreamEncoder(payload(4000), 1)
        val session = PetalScanSession(PetalScanLimits(idleTimeoutMillis = 10_000, absoluteTimeoutMillis = 2_000))
        val first = session.push(render(encoder, 0).toLuma(), 0)
        assertEquals("PKD", first.lanes)
        assertTrue(session.push(render(encoder, 1).toLuma(), 1_500).progress.rank > first.progress.rank)
        // still progressing, but the stream started more than 2 s ago
        val late = session.push(render(encoder, 2).toLuma(), 2_600)
        assertEquals(null, late.error)
        assertTrue(late.progress.rank <= PetalStream.atomsInFrame(2), "rank restarted: ${late.progress.rank}")
        assertNull(late.progress.meta, "frame 2 carries no beacon, so the stream identity is forgotten")
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

    @Test
    fun cleanFramesCountLanesAndDeliverExactlyOnce() {
        val data = payload(200)
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
        assertEquals(stats.frames, stats.laneP)
        assertEquals(stats.frames, stats.laneD)
        session.reset()
        assertNull(session.progress().meta)
    }
}
