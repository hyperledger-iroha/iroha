package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.intOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.array
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.hexBytes
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.number
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.text

/**
 * Decodes the golden camera captures in `fixtures/petal/petal_captures_v3.json`.
 *
 * Every conforming decoder must read the lanes named in `must_decode` and the
 * turn lanes named in `must_turn`, must never report wrong data for any lane,
 * must report the recorded inferred corner, must follow each tracking pair from
 * its first frame into its second, must recover the `must_merge` field lanes of
 * every merge set from the merge of its captures, must never let a session offer
 * a lane of a mixed set, and must reject the negatives. This port mirrors the
 * reference arithmetic, so it must also read exactly the lanes and turn lanes
 * the reference read (`reference_decoded`, `reference_turned`,
 * `reference_tracked`, `reference_merged`). Two captures (`hidden-corner-540p`,
 * `cut-corner-720p`) only decode through an inferred corner, `shadow-band-540p`
 * needs the local read for one field lane and `barrel-480p` the lens distortion
 * search, so they prove that all three are implemented faithfully; the sharp,
 * mirrored, covered and cut-off captures and both tracking pairs read every turn
 * lane, and the soft, blurred, veiled and distorted ones none.
 */
class PetalCaptureFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_captures_v3.json")

    private fun lumaOf(entry: JsonObject, key: String = "luma_zlib_base64", sizes: JsonObject = entry): PetalLuma {
        val data = PetalTestSupport.inflateBase64(entry.text(key))
        return assertNotNull(PetalLuma.fromRaw(sizes.number("width").toInt(), sizes.number("height").toInt(), data))
    }

    /** The recorded `inferred_corner`: a canonical index, or `null`. */
    private fun inferredCornerOf(entry: JsonObject): Int? =
        entry.getValue("inferred_corner").jsonPrimitive.intOrNull

    /** The recorded data of every lane: lane `D`, then the four field lanes. */
    private fun truth(entry: JsonObject): Map<PetalLane, ByteArray> {
        val fields = entry.array("field_data")
        assertEquals(PetalLayout.SECTORS, fields.size)
        val lanes = linkedMapOf(PetalLane.D to entry.hexBytes("d_data"))
        for (lane in PetalLane.FIELDS) lanes[lane] = PetalTestSupport.hex(fields[lane.sector].jsonPrimitive.content)
        return lanes
    }

    /** Checks every decoded lane against the recorded data and returns the lane letters. */
    private fun readLanes(name: String, frame: PetalDecodedFrame, truth: Map<PetalLane, ByteArray>): String {
        for ((lane, expected) in truth) {
            frame.lane(lane)?.let { assertContentEquals(expected, it.data, "$name: lane ${lane.letter} data") }
        }
        return frame.lanes
    }

    /**
     * Reads the turn lanes of a decoded capture (no sector skipped), checks them against the
     * recorded data and the requirements, and returns their sector digits; their atoms go to
     * [assembler] when given.
     */
    private fun readTurnLanes(
        name: String,
        image: PetalLuma,
        frame: PetalDecodedFrame,
        entry: JsonObject,
        assembler: PetalStreamAssembler?,
    ): String {
        val expected = entry.array("turn_data").map { PetalTestSupport.hex(it.jsonPrimitive.content) }
        assertEquals(PetalLayout.SECTORS, expected.size)
        val letters = StringBuilder()
        val packets = ArrayList<PetalAtomPacket>()
        for ((sector, result) in PetalTurns.readTurns(image, frame).withIndex()) {
            if (result == null) continue
            assertContentEquals(expected[sector], result.data, "$name: turn lane $sector data")
            val field = assertNotNull(frame.field(sector), "$name: a turn lane rides on its field lane").data
            val header = PetalLaneHeader(field[0].toInt() and 0xFF, ((field[1].toInt() and 0xFF) shl 8) or (field[2].toInt() and 0xFF))
            packets += assertNotNull(PetalTurns.turnPacket(header, sector, result.data), "$name: turn check $sector")
            letters.append(PetalLane.field(sector).letter)
        }
        if (assembler != null) for (packet in packets) assembler.pushAtoms(packet)
        val must = entry.text("must_turn")
        assertTrue(must.all { it in letters }, "$name: turned $letters, must turn $must")
        assertEquals(entry.text("reference_turned"), letters.toString(), "$name: turn lanes read by the reference")
        return letters.toString()
    }

    @Test
    fun goldenCapturesDecodeAsRecorded() {
        val assembler = PetalStreamAssembler()
        val report = StringBuilder(
            "Petal golden captures (Kotlin/JVM decode times of decodes 1, 2, 3 and the best of 6; turn reads 1 and the best of 6):\n",
        )
        assertEquals(12, doc.array("captures").size)
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val image = lumaOf(capture)
            val started = System.nanoTime()
            val result = PetalDecoder.decode(image)
            val first = (System.nanoTime() - started) / 1e6
            val decoded = assertNotNull(result.frame, "$name: ${result.error}")
            assertEquals(capture.getValue("mirrored").jsonPrimitive.boolean, decoded.mirrored, "$name: mirror flag")
            val read = readLanes(name, decoded, truth(capture))
            val must = capture.text("must_decode")
            assertTrue(must.all { it in read }, "$name: read $read, must read $must")
            assertEquals(capture.text("reference_decoded"), read, "$name: lanes read by the reference")
            assertEquals(inferredCornerOf(capture), decoded.inferredCorner, "$name: inferred corner")
            decoded.feed(assembler)
            val turnStarted = System.nanoTime()
            val turned = readTurnLanes(name, image, decoded, capture, assembler)
            val firstTurn = (System.nanoTime() - turnStarted) / 1e6
            // timing: the JIT warms up over the first decodes
            val timings = DoubleArray(6)
            timings[0] = first
            for (run in 1 until timings.size) {
                val again = System.nanoTime()
                val repeated = PetalDecoder.decode(image)
                timings[run] = (System.nanoTime() - again) / 1e6
                assertEquals(decoded.lanes, assertNotNull(repeated.frame).lanes, "$name: decoding is deterministic")
            }
            val reader = PetalTurnReader()
            val results = arrayOfNulls<PetalLaneResult>(PetalLayout.SECTORS)
            var turnBest = Double.MAX_VALUE
            repeat(6) {
                val again = System.nanoTime()
                reader.read(image, decoded, BooleanArray(PetalLayout.SECTORS), results)
                turnBest = minOf(turnBest, (System.nanoTime() - again) / 1e6)
                assertEquals(turned, results.withIndex().filter { it.value != null }.joinToString("") { "${it.index}" }, "$name: deterministic")
            }
            report.append(
                String.format(
                    "  %-22s %4dx%-4d lanes %-5s (reference %-5s, required %-5s) turned %-4s  " +
                        "#1 %6.1f ms  #2 %6.1f ms  #3 %6.1f ms  best %6.1f ms  turns #1 %6.1f ms  best %6.1f ms%n",
                    name, image.width, image.height, decoded.lanes, capture.text("reference_decoded"), must, turned,
                    timings[0], timings[1], timings[2], timings.minOrNull()!!, firstTurn, turnBest,
                ),
            )
        }
        println(report)
        // captures of different frames of the same stream accumulate in one assembler
        assertTrue(assembler.progress().atomsReceived > 100)
    }

    @Test
    fun goldenTracksFollowThePoseIntoTheNextFrame() {
        val tracks = doc.array("tracks")
        assertEquals(2, tracks.size)
        val report = StringBuilder("Petal golden tracks (Kotlin/JVM, second frame: best of 6 full decodes vs best of 6 tracks):\n")
        for (element in tracks) {
            val pair = element.jsonObject
            val name = pair.text("name")
            val from = lumaOf(pair, "from_luma_zlib_base64")
            val to = lumaOf(pair, "to_luma_zlib_base64")
            val first = PetalDecoder.decode(from)
            val previous = assertNotNull(first.frame, "$name: first frame: ${first.error}")
            val followed = assertNotNull(PetalDecoder.track(to, previous), "$name: tracking lost the code")
            val read = readLanes(name, followed, truth(pair))
            val must = pair.text("must_track")
            assertTrue(must.all { it in read }, "$name: tracked $read, must track $must")
            assertEquals(pair.text("reference_tracked"), read, "$name: lanes the reference tracked")
            assertEquals(inferredCornerOf(pair), followed.inferredCorner, "$name: inferred corner")
            assertEquals(previous.rotation, followed.rotation, "$name: tracking keeps the orientation")
            assertEquals(previous.mirrored, followed.mirrored, "$name: tracking keeps the mirror flag")
            val turned = readTurnLanes(name, to, followed, pair, null)
            // timing: tracking skips the finder search
            var decodeBest = Double.MAX_VALUE
            var trackBest = Double.MAX_VALUE
            repeat(6) {
                var started = System.nanoTime()
                PetalDecoder.decode(to)
                decodeBest = minOf(decodeBest, (System.nanoTime() - started) / 1e6)
                started = System.nanoTime()
                assertEquals(followed.lanes, assertNotNull(PetalDecoder.track(to, previous)).lanes, "$name: deterministic")
                trackBest = minOf(trackBest, (System.nanoTime() - started) / 1e6)
            }
            report.append(
                String.format(
                    "  %-22s lanes %-5s (reference %-5s) turned %-4s inferred %-4s  decode %6.1f ms  track %6.1f ms%n",
                    name, followed.lanes, pair.text("reference_tracked"), turned, followed.inferredCorner, decodeBest, trackBest,
                ),
            )
        }
        println(report)
    }

    /**
     * Decodes every capture of a merge set as recorded and pushes it through a fresh session (30 fps);
     * returns the merge of their field evidence, the lanes the session merged and its counters.
     */
    private fun mergeSet(set: JsonObject): Triple<Array<PetalLaneResult?>, String, PetalScanStats> {
        val name = set.text("name")
        val truth = truth(set)
        val evidence = ArrayList<PetalDecoder.FieldEvidence>()
        val session = PetalScanSession()
        val merged = StringBuilder()
        for ((index, element) in set.array("captures").withIndex()) {
            val capture = element.jsonObject
            val image = lumaOf(capture, sizes = set)
            val decoded = assertNotNull(PetalDecoder.decode(image).frame, "$name capture $index")
            val read = readLanes("$name capture $index", decoded, truth)
            val must = capture.text("must_decode")
            assertTrue(must.all { it in read }, "$name capture $index: read $read, must read $must")
            assertEquals(capture.text("reference_decoded"), read, "$name capture $index: lanes read alone")
            evidence += assertNotNull(
                PetalDecoder.fieldEvidence(image, decoded, PetalDecodeOptions.DEFAULT, null),
                "$name capture $index: field evidence",
            )
            merged.append(session.push(image, index * 33L).merged)
        }
        val results = PetalDecoder.mergeFields(evidence, BooleanArray(PetalLayout.SECTORS), PetalWorkspace())
        return Triple(results, merged.toString(), session.stats())
    }

    /** Checks merged field lanes against the recorded data and returns their sector digits. */
    private fun mergedLetters(name: String, results: Array<PetalLaneResult?>, set: JsonObject): String {
        val truth = truth(set)
        val letters = StringBuilder()
        for (lane in PetalLane.FIELDS) {
            val result = results[lane.sector] ?: continue
            assertContentEquals(truth.getValue(lane), result.data, "$name: merged lane ${lane.letter} data")
            letters.append(lane.letter)
        }
        return letters.toString()
    }

    private fun mergedCounts(results: Array<PetalLaneResult?>): String =
        PetalLane.FIELDS.joinToString(" ") { lane ->
            "${lane.letter}:" + (results[lane.sector]?.let { "${it.corrected}/${it.erasures}" } ?: "-")
        }

    @Test
    fun goldenMergeSetsRecoverLanesNoCaptureReadsAlone() {
        val merges = doc.array("merges")
        assertEquals(1, merges.size)
        for (element in merges) {
            val set = element.jsonObject
            val name = set.text("name")
            val (results, sessionMerged, stats) = mergeSet(set)
            assertEquals(set.text("reference_merged"), mergedLetters(name, results, set), "$name: merged lanes")
            for (letter in set.text("must_merge")) {
                assertTrue(letter in sessionMerged, "$name: the session did not recover lane $letter (merged $sessionMerged)")
            }
            assertTrue(stats.merges > 0, "$name: no merge")
            // what the Rust reference merges, bit for bit: the four captures' mean, and the session
            // merging two, then three, then four held captures
            assertEquals(referenceMerges.getValue(name), mergedCounts(results), "$name: corrected/erasures of the merge")
            assertEquals(referenceSessionMerges.getValue(name), sessionMerged, "$name: lanes the session merged")
            assertEquals(3L, stats.merges, name)
            assertEquals(sessionMerged.length.toLong(), stats.mergedLanes, name)
            // merged lanes are not single-capture reads: no turn lane rides on them
            assertEquals(0L, stats.turnLanes, name)
        }
    }

    @Test
    fun goldenMixedSetsNeverOfferTheOtherFramesLanes() {
        val mixed = doc.array("mixed_merges")
        assertEquals(1, mixed.size)
        for (element in mixed) {
            val set = element.jsonObject
            val name = set.text("name")
            val (results, sessionMerged, stats) = mergeSet(set)
            // the merge itself reads the petal field, which shows another frame than the rings
            val ringsFrame = set.hexBytes("d_data").copyOfRange(1, 3)
            for (result in results.filterNotNull()) {
                assertTrue(!result.data.copyOfRange(1, 3).contentEquals(ringsFrame), "$name: a merged lane names the rings' frame")
            }
            assertEquals(set.number("petals_frame"), set.number("frame") + 1, name)
            assertEquals(set.text("reference_merged"), mergedLetters(name, results, set), "$name: merged lanes")
            assertEquals(referenceMerges.getValue(name), mergedCounts(results), "$name: corrected/erasures of the merge")
            assertTrue(stats.merges > 0, "$name: no merge was tried")
            assertEquals("", sessionMerged, "$name: the session offered another frame's lanes")
            assertEquals(0L, stats.mergedLanes, name)
        }
    }

    /**
     * What the Rust reference read from a capture: the `decode` pose (homography and distortion
     * bits), the orientation, corrected/erasures per lane and per turn lane (`read_turns` with no
     * sector skipped) and the blur its turn reader fitted (an index into [PetalTurns.TURN_BLURS]).
     */
    private class ReferenceReading(
        val homographyBits: List<String>,
        val distortionBits: String,
        val rotation: Int,
        val mirrored: Boolean,
        val lanes: String,
        val turnLanes: String,
        val fittedBlur: Int,
    )

    /**
     * Intermediates dumped from the Rust reference (`iroha_petal::decode::decode`, `track` and
     * `turns::read_turns` on these captures, macOS/arm64). The spec does not require
     * bit-identical intermediates across platforms; this port follows the reference's IEEE double
     * order of operations with deterministic `StrictMath`, and pinning the values guards it
     * against arithmetic drift. (The turn reader's tables use `exp`, whose last bit can differ
     * between libraries: one of the 36 template probes differs by one ulp, and no decision does.)
     */
    private val referenceReadings = mapOf(
        "clean-512" to ReferenceReading(
            listOf(
                "3fe00029696ba5f9", "3f0461138c40a62c", "bf6ec02f3ae3b3f1",
                "3ef049ad824b297f", "3fe000713cf8436d", "bfa25a1ee0979718",
                "3e618583a96052df", "3e83c8d1e38b8d87", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 0,
        ),
        "modern-720p-rotated" to ReferenceReading(
            listOf(
                "3fd9dfeb4809c413", "bfd06566a21ce590", "4082f4fefe605541",
                "3fd31e4350d1fab6", "3fd90bc0e054b289", "401fa98edc6722ca",
                "3f0531825a194e74", "3f0952cfffe823bd", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
        ),
        "legacy-540p-tilted" to ReferenceReading(
            listOf(
                "bfd552bf1d10ecf4", "3fca44bb710cebcb", "408340e0409ad1c2",
                "bfbd1d31346c0af9", "bfd5ee9ec53642d5", "40806d13d3424e99",
                "3f181867e97a353c", "3f1c8f0bd33f6f2b", "3ff0000000000000",
            ),
            "0000000000000000", 2, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
        ),
        "soft-480p-blur1.9" to ReferenceReading(
            listOf(
                "bfc2215ff6ce0cfc", "bfd48045d2a13568", "4080662b2634cbbf",
                "3fd12a3a40cd55ce", "bfc1506d167c0179", "406287e0ff7c8401",
                "bf1c535776e1421a", "bf1651400c81b6b8", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:2/0 1:4/0 2:9/0 3:3/0", "0:- 1:- 2:- 3:-", 5,
        ),
        "small-480p" to ReferenceReading(
            listOf(
                "3fb74b7c4736e59e", "3fd5b9a944e6d7a8", "4058e74af36bae89",
                "bfd5ba00c1379c29", "3fb746496cce8cf7", "4076f4121d6166b4",
                "3e81d4a64a782cba", "be889c51d6a579d5", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 4,
        ),
        "selfie-mirrored-540p" to ReferenceReading(
            listOf(
                "bfd8d0ae55b75ad1", "3fb60e52b98c3e2e", "40838bb679a30ccd",
                "3fba9a2775678d05", "3fd8200887a075e5", "4032cff70cb4d374",
                "3e6748fdc5046750", "bf031d9551fa441e", "3ff0000000000000",
            ),
            "0000000000000000", 1, true, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
        ),
        "overexposed-540p" to ReferenceReading(
            listOf(
                "3fd64a9087ac475b", "bfc41fa64d477da1", "4079b04613fff1c5",
                "3fc7b9b113e0c080", "3fd5d77052143a84", "40153ac09a8d4a13",
                "3f01f32d5951d0d3", "3f064ea70c6c15c4", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:11/0 2:12/2 3:-", 3,
        ),
        "veiled-720p" to ReferenceReading(
            listOf(
                "bfc03614b2d9fb44", "3fe339347ddf68ef", "407ed88487f21eb9",
                "bfdf4c0483869f56", "bfc0abb61dcb3107", "40861c3f23cfa28a",
                "3f18fb7538983040", "3f1d73cd71131e4f", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 2,
        ),
        "shadow-band-540p" to ReferenceReading(
            listOf(
                "3fdc6a34efaeb430", "bfafc710429cedb6", "4071c80378c4d449",
                "3fafeec8afdd7598", "3fdc6aeb254cc3ce", "4025a9168e8b7aed",
                "be734d2367f3656f", "3e9d792aec1568a1", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:- 0:0/0 1:15/11 2:10/0 3:11/0", "0:5/0 1:6/0 2:3/0 3:9/0", 2,
        ),
        "hidden-corner-540p" to ReferenceReading(
            listOf(
                "bfd7b34e06e88606", "bfbd86e6ef983d51", "40876e8b718eec3a",
                "3fc35575e4fa5208", "bfd800a93c46417d", "4078c99382115a81",
                "3f03552435ecb4b5", "3f0a1af003e9bb76", "3ff0000000000000",
            ),
            "0000000000000000", 2, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
        ),
        "cut-corner-720p" to ReferenceReading(
            listOf(
                "3fd64489a45d0744", "bfd64524e85e299a", "4084ccd43cfb5614",
                "3fd645375d87ddff", "3fd6463abc465805", "405683498e879027",
                "be74a1623ba56aed", "3e70d67aeacce137", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 1,
        ),
        "barrel-480p" to ReferenceReading(
            listOf(
                "3fd4be433a5f1ba6", "bfba57768c12e7f9", "406acaf38e7d1c31",
                "3fbe3425a47829ba", "3fd566875cad8e50", "402c1c2da56b7f9d",
                "3e72f7bdaf03187f", "3f0835d13ba8f65b", "3ff0000000000000",
            ),
            "bfbeb851eb851eb8", 0, false, "D:0/0 0:0/0 1:0/0 2:1/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
        ),
    )

    private val referenceTracks = mapOf(
        "steady-hand-540p" to ReferenceReading(
            listOf(
                "3fd3fe39804549ea", "bfc6d6f85122d649", "407ab755660467c6",
                "3fc8fc4b89815f46", "3fd483207d176319", "401d0986f9303303",
                "3e5153069e35fd1f", "3f01806a07e11c54", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:1/0 1:0/0 2:0/0 3:0/0", 2,
        ),
        "thumb-arrives-540p" to ReferenceReading(
            listOf(
                "3fd4e7b32a42ecae", "bfc6cf963f451f5c", "407976e1078e6955",
                "3fc9223ae4f7f44e", "3fd57317a9c8b75c", "40161e5a7d6f1043",
                "3e9affe577aca827", "3f0371e6ae3c42b8", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:1/0 1:0/0 2:0/0 3:0/0", 2,
        ),
    )

    /** The four-capture merge of each merge set as the Rust reference reads it (`merge_fields`). */
    private val referenceMerges = mapOf(
        "noisy-480p" to "0:6/0 1:7/0 2:16/11 3:11/0",
        "mixed-frames-480p" to "0:7/0 1:7/0 2:- 3:7/0",
    )

    /** The lanes a reference `ScanSession` merges from the noisy set, in the order it offers them. */
    private val referenceSessionMerges = mapOf("noisy-480p" to "0123")

    private fun bits(value: Double): String = String.format("%016x", java.lang.Double.doubleToRawLongBits(value))

    private fun laneCounts(frame: PetalDecodedFrame): String =
        PetalLane.ALL.joinToString(" ") { lane ->
            "${lane.letter}:" + (frame.lane(lane)?.let { "${it.corrected}/${it.erasures}" } ?: "-")
        }

    private fun turnCounts(results: List<PetalLaneResult?>): String =
        results.withIndex().joinToString(" ") { (sector, result) ->
            "$sector:" + (result?.let { "${it.corrected}/${it.erasures}" } ?: "-")
        }

    private fun assertReading(name: String, expected: ReferenceReading, image: PetalLuma, frame: PetalDecodedFrame) {
        assertEquals(expected.homographyBits, frame.pose.homography.toArray().map(::bits), "$name: homography")
        assertEquals(expected.distortionBits, bits(frame.pose.distortion), "$name: lens distortion")
        assertEquals(expected.rotation, frame.rotation, "$name: rotation")
        assertEquals(expected.mirrored, frame.mirrored, "$name: mirrored")
        assertEquals(expected.lanes, laneCounts(frame), "$name: corrected/erasures per lane")
        assertEquals(expected.turnLanes, turnCounts(PetalTurns.readTurns(image, frame)), "$name: corrected/erasures per turn lane")
        val observed = assertNotNull(PetalTurns.observedTurns(image, frame), "$name: carriers")
        assertEquals(PetalTurns.TURN_BLURS[expected.fittedBlur], observed.blur, "$name: fitted blur")
        assertEquals(PetalLayout.SECTORS * PetalTurns.TURN_CARRIERS, observed.carriers.size, "$name: carriers")
    }

    @Test
    fun decoderArithmeticMatchesTheReferenceBitForBit() {
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val image = lumaOf(capture)
            assertReading(name, referenceReadings.getValue(name), image, assertNotNull(PetalDecoder.decode(image).frame, name))
        }
        for (element in doc.array("tracks")) {
            val pair = element.jsonObject
            val name = pair.text("name")
            val previous = assertNotNull(PetalDecoder.decode(lumaOf(pair, "from_luma_zlib_base64")).frame, name)
            val to = lumaOf(pair, "to_luma_zlib_base64")
            assertReading(name, referenceTracks.getValue(name), to, assertNotNull(PetalDecoder.track(to, previous), name))
        }
        // the lens distortion search found the barrel
        assertTrue(referenceReadings.getValue("barrel-480p").distortionBits != bits(0.0))
    }

    @Test
    fun theLocalReadRescuesWhatTheLevelReadLoses() {
        val workspace = PetalWorkspace()
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val image = lumaOf(capture)
            val decoded = assertNotNull(PetalDecoder.decode(image).frame, name)
            val reference = assertNotNull(
                PetalDecoder.referenceLevels(image, decoded.pose, decoded.inferredCorner ?: PetalDecoder.NO_CORNER),
                "$name: reference levels",
            )
            val levels = DoubleArray(PetalLayout.FIELD_CELLS)
            PetalDecoder.fieldLevels(image, decoded.pose, reference, levels)
            val level = arrayOfNulls<PetalLaneResult>(PetalLayout.SECTORS)
            PetalDecoder.readSectors(levels, 0, level, workspace)
            val levelLanes = PetalLane.FIELDS.filter { level[it.sector] != null }.joinToString("") { it.letter.toString() }
            // what the Rust reference's level read alone decodes under the decoded pose
            assertEquals(if (name == "shadow-band-540p") "012" else "0123", levelLanes, "$name: lanes of the level read")
            // decode keeps every lane the level read has and takes the others from the local read
            for (lane in PetalLane.FIELDS) {
                level[lane.sector]?.let { assertEquals(it, decoded.lane(lane), "$name: $lane from the level read") }
            }
        }
    }

    @Test
    fun negativeCapturesAreRejected() {
        val negatives = doc.array("negatives")
        assertEquals(2, negatives.size)
        for (element in negatives) {
            val negative = element.jsonObject
            val result = PetalDecoder.decode(lumaOf(negative))
            assertNull(result.frame, negative.text("name"))
            assertEquals(PetalDecodeError.NO_FINDERS, result.error, negative.text("name"))
        }
    }

    @Test
    fun recordedLaneDataIsAValidCodewordOfItsStream() {
        val payload = PetalTestSupport.hex(doc.text("payload_hex"))
        val encoder = PetalStreamEncoder(payload, doc.number("payload_kind").toInt())
        val stream = PetalTestSupport.loadFixture("petal_stream_v3.json").array("streams")
            .map { it.jsonObject }.single { it.text("name") == "one-pass" }
        assertContentEquals(stream.hexBytes("payload_hex"), payload)
        fun checkTurns(name: String, entry: JsonObject, data: PetalFrameLanes) {
            val turns = entry.array("turn_data").map { PetalTestSupport.hex(it.jsonPrimitive.content) }
            for (sector in 0 until PetalLayout.SECTORS) {
                assertContentEquals(turns[sector], data.turn(sector), "$name T$sector")
                val word = PetalTurns.encodeTurnLane(sector, turns[sector])
                assertContentEquals(turns[sector], PetalTurns.decodeTurnLaneCounted(sector, word).data)
            }
        }
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val data = encoder.laneData(capture.number("frame").toInt())
            for ((lane, expected) in truth(capture)) {
                assertContentEquals(expected, data.lane(lane), "${capture.text("name")} $lane")
                assertContentEquals(expected, PetalLanes.decodeLane(lane, PetalLanes.encodeLane(lane, expected)))
            }
            checkTurns(capture.text("name"), capture, data)
        }
        // the lane data of a tracking pair is that of its second frame
        for (element in doc.array("tracks")) {
            val pair = element.jsonObject
            assertEquals(pair.number("from_frame") + 1, pair.number("to_frame"))
            val data = encoder.laneData(pair.number("to_frame").toInt())
            for ((lane, expected) in truth(pair)) assertContentEquals(expected, data.lane(lane), "${pair.text("name")} $lane")
            checkTurns(pair.text("name"), pair, data)
        }
        // a merge set records the lanes of its shown frame; a mixed set its rings' frame and petals' frame
        for (element in doc.array("merges")) {
            val set = element.jsonObject
            val data = encoder.laneData(set.number("frame").toInt())
            for ((lane, expected) in truth(set)) assertContentEquals(expected, data.lane(lane), "${set.text("name")} $lane")
        }
        for (element in doc.array("mixed_merges")) {
            val set = element.jsonObject
            val truth = truth(set)
            assertContentEquals(encoder.laneData(set.number("frame").toInt()).d(), truth.getValue(PetalLane.D))
            val petals = encoder.laneData(set.number("petals_frame").toInt())
            for (lane in PetalLane.FIELDS) assertContentEquals(petals.lane(lane), truth.getValue(lane), "${set.text("name")} $lane")
            assertNotEquals(set.number("frame"), set.number("petals_frame"))
        }
    }
}
