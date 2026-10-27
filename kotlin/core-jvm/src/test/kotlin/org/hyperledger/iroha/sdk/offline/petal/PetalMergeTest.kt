package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Same-frame averaging of field evidence and bounded session intake. */
class PetalMergeTest {
    private val options = PetalDecodeOptions.DEFAULT
    private fun encoder() = PetalStreamEncoder(PetalTestSupport.payload(700, 2), 2)
    private fun render(encoder: PetalStreamEncoder, frame: Int) = PetalTestSupport.renderLuma(encoder, frame, 768, 2)
    private fun evidence(image: PetalLuma, frame: PetalDecodedFrame) =
        assertNotNull(PetalDecoder.fieldEvidence(image, frame, options, null))
    private fun merge(captures: Collection<PetalDecoder.FieldEvidence>, skip: BooleanArray = BooleanArray(PetalLayout.SECTORS)) =
        PetalDecoder.mergeFields(captures, skip, PetalWorkspace())
    private fun withLanes(
        pose: PetalDecodedFrame,
        d: PetalLaneResult? = null,
        fields: List<PetalLaneResult?> = listOf(null, null, null, null),
        inferred: Int? = pose.inferredCorner,
    ) = PetalDecodedFrame(pose.pose, pose.rotation, pose.mirrored, d, fields, inferred)

    @Test
    fun oneCaptureReadsExactlyTheFieldLanesTheDecoderRead() {
        val image = render(encoder(), 5)
        val shadow = image.data()
        for (y in 0 until image.height) for (x in image.width * 7 / 20 until image.width * 3 / 5) {
            val at = y * image.width + x
            shadow[at] = PetalNumerics.roundHalfAway((shadow[at].toInt() and 0xFF) * 0.4).toInt().toByte()
        }
        val hidden = image.data()
        val scale = image.width / 1024.0
        for (y in 0 until image.height) for (x in 0 until image.width) {
            val dx = x + 0.5 - PetalLayout.finderCenterX(3) * scale
            val dy = y + 0.5 - PetalLayout.finderCenterY(3) * scale
            if (dx * dx + dy * dy <= 75.0 * scale * 75.0 * scale) hidden[y * image.width + x] = 0
        }
        val mirrored = ByteArray(hidden.size) { at -> hidden[(at / image.width) * image.width + image.width - 1 - at % image.width] }
        for ((name, data) in listOf("clean" to image.data(), "shadow" to shadow, "hidden" to hidden, "mirrored" to mirrored)) {
            val capture = assertNotNull(PetalLuma.fromRaw(image.width, image.height, data))
            val frame = assertNotNull(PetalDecoder.decode(capture).frame, name)
            assertTrue(frame.fields.any { it != null }, name)
            // the shadowed columns cost a field lane, exactly as the reference decoder reads them
            // (after a lens search that keeps k = 0); the hard edge also defeats the local read
            if (name == "shadow") assertEquals("D123" to 0.0, frame.lanes to frame.pose.distortion)
            assertEquals(frame.fields, merge(listOf(evidence(capture, frame))).toList(), "$name: field lanes, including correction counts")
        }
    }

    @Test
    fun heldEvidenceOwnsItsLevels() {
        val image = render(encoder(), 5)
        val workspace = PetalWorkspace()
        val frame = assertNotNull(PetalDecoder.decode(image, options, workspace).frame)
        val capture = assertNotNull(PetalDecoder.fieldEvidence(image, frame, options, null))
        // decoding another picture with the same workspace reuses every scratch buffer
        assertNotNull(PetalDecoder.decode(render(encoder(), 6), options, workspace).frame)
        assertEquals(frame.fields, merge(listOf(capture)).toList(), "later scratch reuse cannot change held evidence")
        assertTrue(capture.levels !== workspace.fieldReads[0].levels && capture.levels !== workspace.fieldReads[1].levels)
        assertTrue(merge(emptyList()).all { it == null })
        assertNull(PetalDecoder.fieldEvidence(PetalLuma(image.width, image.height), frame, options, null))
        for (corner in listOf(-1, 4, 255)) assertNull(PetalDecoder.fieldEvidence(image, withLanes(frame, inferred = corner), options, null))
    }

    @Test
    fun tornUnkeyedAndUnrelatedTagCapturesNeverShareEvidence() {
        val encoder = encoder()
        val image = render(encoder, 11)
        val pose = assertNotNull(PetalDecoder.decode(image).frame)
        val session = PetalScanSession()
        val rings = PetalLaneResult(encoder.laneData(10).d(), 0, 0)
        assertEquals("", session.merge(image, withLanes(pose, rings, listOf(pose.field(0), null, null, null))))
        assertEquals("", session.merge(image, withLanes(pose)))
        assertTrue(session.held.isEmpty(), "torn and unkeyed captures are never held")
        session.merge(image, withLanes(pose, rings))
        val other = rings.data
        other[0] = (other[0].toInt() xor 1).toByte()
        session.merge(image, withLanes(pose, PetalLaneResult(other, 0, 0)))
        assertEquals(2, session.held.size, "different stream tags cannot share evidence")
        assertEquals(0L, session.stats().merges)
    }

    @Test
    fun aShownFrameOnlyGainsMissingLanesAndReleasesEvidenceWhenComplete() {
        val encoder = encoder()
        val image = render(encoder, 5)
        val pose = assertNotNull(PetalDecoder.decode(image).frame)
        assertEquals("D0123", pose.lanes)
        val session = PetalScanSession()
        val partial = withLanes(pose, pose.d, listOf(pose.field(0), pose.field(1), null, null))
        assertEquals("", session.merge(image, partial))
        assertEquals("23", session.merge(image, partial))
        assertEquals(2L, session.stats().mergedLanes)
        assertTrue(session.held.first.evidence.isEmpty())
        assertTrue(session.held.first.done.all { it })
        repeat(3) { assertEquals("", session.merge(image, partial)) }
        assertEquals(1L, session.stats().merges, "complete field lanes are never merged or offered again")
    }

    @Test
    fun aFullWindowReusesTheOldestCapturesBuffer() {
        val encoder = encoder()
        val image = render(encoder, 11)
        val pose = assertNotNull(PetalDecoder.decode(image).frame)
        // lane D of another frame: the merges are refused, so the shown frame never completes
        val torn = withLanes(pose, PetalLaneResult(encoder.laneData(10).d(), 0, 0))
        val session = PetalScanSession()
        repeat(PetalScanSession.MERGE_CAPTURES) { session.merge(image, torn) }
        val held = session.held.first
        val oldest = held.evidence.first.levels
        val fresh = evidence(image, pose).levels
        session.merge(image, torn)
        assertEquals(PetalScanSession.MERGE_CAPTURES, held.evidence.size)
        assertTrue(held.evidence.last.levels === oldest, "the evicted capture's buffer holds the newest evidence")
        assertContentEquals(fresh, held.evidence.last.levels)
        assertContentEquals(encoder.laneData(11).field(3), assertNotNull(merge(held.evidence)[3]).data)
    }

    /** A real beacon or atom lane `D` with deterministic uncorrectable field words. */
    private fun dOnly(encoder: PetalStreamEncoder, frame: Int): PetalLuma {
        val rng = PetalXorshift32(97)
        val cells = PetalFrameCells.fromWords(
            encoder.words(frame).d(),
            List(PetalLayout.SECTORS) { ByteArray(PetalLanes.FIELD_WORD) { rng.nextByte().toByte() } },
        )
        return PetalRenderer.render(cells, PetalRenderOptions(768, 2)).toLuma().also {
            assertEquals("D", assertNotNull(PetalDecoder.decode(it).frame).lanes)
        }
    }

    @Test
    fun acceptedMetadataReplacementDropsSameTagSameFrameEvidenceAndDoneFlags() {
        // two distinct 100-byte payloads whose CRC low byte is 61
        val oldPayload = ByteArray(100).also { it[3] = 49 }
        val newPayload = ByteArray(100).also { it[3] = 128.toByte() }
        val old = PetalStreamEncoder(oldPayload, 2)
        val next = PetalStreamEncoder(newPayload, 2)
        assertEquals(61, old.meta.tag)
        assertEquals(old.meta.tag, next.meta.tag)
        assertTrue(old.meta != next.meta)
        val session = PetalScanSession()
        session.push(render(old, 0), 0)
        assertTrue(session.held.first.done.all { it })
        val image = dOnly(next, 0)
        session.push(image, 33)
        assertEquals(old.meta, session.progress().meta, "one conflicting beacon does not replace the stream")
        session.push(image, 66)
        assertEquals(next.meta, session.progress().meta)
        assertEquals(1, session.held.size)
        val fresh = session.held.first
        assertTrue(fresh.done.none { it }, "the previous payload's done flags cannot suppress recovery")
        assertEquals(1, fresh.evidence.size, "no prior payload's evidence survives replacement")
        val partial = assertNotNull(PetalDecoder.decode(image).frame)
        val recovered = StringBuilder()
        val clean = render(next, 0)
        repeat(PetalScanSession.MERGE_CAPTURES) { recovered.append(session.merge(clean, partial)) }
        assertEquals("0123", recovered.toString())
        assertContentEquals(newPayload, assertNotNull(session.push(render(next, 1), 99).completed).payload)
    }

    @Test
    fun theFirstBeaconPreservesEarlierUnidentifiedFrameEvidence() {
        val encoder = encoder()
        val session = PetalScanSession()
        session.push(dOnly(encoder, 1), 0)
        assertNull(session.progress().meta)
        val held = session.held.single()
        assertEquals(1, held.shown.frame)
        val earlier = held.evidence.single()
        session.push(dOnly(encoder, 0), 33)
        assertEquals(encoder.meta, session.progress().meta)
        val preserved = session.held.single { it.shown.frame == 1 }
        assertTrue(preserved === held && preserved.evidence.single() === earlier)
    }
}
