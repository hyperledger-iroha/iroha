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
 * Decodes the golden camera captures in `fixtures/petal/petal_captures_v5.json`.
 *
 * Every conforming decoder must read the lanes named in `must_decode`, the turn
 * lanes named in `must_turn` and the kana lanes named in `must_kana`, must never
 * report wrong data for any lane,
 * must report the recorded inferred corner, must follow each tracking pair from
 * its first frame into its second, must recover the `must_merge` field lanes of
 * every merge set from the merge of its captures, must never let a session offer
 * a lane of a mixed set, and must reject the negatives. This port mirrors the
 * reference arithmetic, so it must also read exactly the lanes, turn lanes and
 * kana lanes the reference read (`reference_decoded`, `reference_turned`,
 * `reference_kana`, `reference_tracked`, `reference_merged`). Two captures (`hidden-corner-540p`,
 * `cut-corner-720p`) only decode through an inferred corner, `shadow-band-540p`
 * needs the local read for one field lane and `barrel-480p` the lens distortion
 * search, so they prove that all three are implemented faithfully; the sharp,
 * mirrored, covered and cut-off captures and both tracking pairs read every turn
 * lane, and the soft, blurred, veiled and distorted ones none. The four upright
 * captures see both katakana bands: the sharp, older and soft 720p cameras read
 * all four kana lanes and the 480p one three.
 */
class PetalCaptureFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_captures_v5.json")

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

    /** The header a decoded capture's lanes name: its first atom packet's, or its beacon's. */
    private fun headerOf(frame: PetalDecodedFrame): PetalLaneHeader =
        frame.atomPackets().firstOrNull()?.header ?: assertNotNull(frame.beacon(), "a decoded capture names its frame").header

    /**
     * Reads the kana lanes of a decoded capture under the header its lanes name (no band skipped),
     * checks them against the recorded data and the requirements, and returns their names; their
     * atoms go to [assembler] when given.
     */
    private fun readKanaLanes(
        name: String,
        image: PetalLuma,
        frame: PetalDecodedFrame,
        entry: JsonObject,
        assembler: PetalStreamAssembler?,
    ): String {
        val expected = entry.array("kana_data").map { PetalTestSupport.hex(it.jsonPrimitive.content) }
        assertEquals(PetalKanaLane.ALL.size, expected.size)
        val names = StringBuilder()
        val packets = ArrayList<PetalAtomPacket>()
        for ((index, result) in PetalKana.readKana(image, frame, headerOf(frame)).withIndex()) {
            if (result == null) continue
            val lane = PetalKanaLane.ALL[index]
            assertContentEquals(expected[index], result.result.data, "$name: kana lane ${lane.name} data")
            packets += assertNotNull(PetalKana.kanaPacket(result.header, lane, result.result.data), "$name: kana check ${lane.name}")
            names.append(lane.name)
        }
        if (assembler != null) for (packet in packets) assembler.pushAtoms(packet)
        val must = entry["must_kana"]?.jsonPrimitive?.content ?: ""
        for (required in must.chunked(2)) {
            assertTrue(names.chunked(2).contains(required), "$name: kana $names, must read $must")
        }
        assertEquals(entry.text("reference_kana"), names.toString(), "$name: kana lanes read by the reference")
        return names.toString()
    }

    @Test
    fun goldenCapturesDecodeAsRecorded() {
        val assembler = PetalStreamAssembler()
        val report = StringBuilder(
            "Petal golden captures (Kotlin/JVM decode times of decodes 1, 2, 3 and the best of 6; turn and kana reads 1 and the best of 6):\n",
        )
        assertEquals(16, doc.array("captures").size)
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
            val kanaStarted = System.nanoTime()
            val kana = readKanaLanes(name, image, decoded, capture, assembler)
            val firstKana = (System.nanoTime() - kanaStarted) / 1e6
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
            val kanaReader = PetalKanaReader()
            val kanaResults = arrayOfNulls<PetalKanaResult>(PetalKanaLane.ALL.size)
            var kanaBest = Double.MAX_VALUE
            repeat(6) {
                val again = System.nanoTime()
                kanaReader.read(image, decoded, headerOf(decoded), BooleanArray(PetalLayout.KANA_BANDS), kanaResults)
                kanaBest = minOf(kanaBest, (System.nanoTime() - again) / 1e6)
                val names = kanaResults.withIndex().filter { it.value != null }.joinToString("") { PetalKanaLane.ALL[it.index].name }
                assertEquals(kana, names, "$name: kana reading is deterministic")
            }
            report.append(
                String.format(
                    "  %-22s %4dx%-4d lanes %-5s (reference %-5s, required %-5s) turned %-4s kana %-8s  " +
                        "#1 %6.1f ms  #2 %6.1f ms  #3 %6.1f ms  best %6.1f ms  turns #1 %6.1f ms  best %6.1f ms  " +
                        "kana #1 %6.1f ms  best %6.1f ms%n",
                    name, image.width, image.height, decoded.lanes, capture.text("reference_decoded"), must, turned, kana,
                    timings[0], timings[1], timings[2], timings.minOrNull()!!, firstTurn, turnBest, firstKana, kanaBest,
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
            readKanaLanes(name, to, followed, pair, null)
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
     * sector skipped), the blur its turn reader fitted (an index into [PetalTurns.TURN_BLURS]),
     * corrected/erasures per kana lane (`read_kana` with no band skipped) and per band what
     * `observe_band` saw: the blur it chose (an index into [PetalKana.KANA_BLURS]), the pilots it
     * found and the data tiles it read, or `-`.
     */
    private class ReferenceReading(
        val homographyBits: List<String>,
        val distortionBits: String,
        val rotation: Int,
        val mirrored: Boolean,
        val lanes: String,
        val turnLanes: String,
        val fittedBlur: Int,
        val kanaLanes: String,
        val kanaBands: String,
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
                "3fdfff8ddf52cda6", "bee412d9ed20c9b7", "3f97a166fceb58cc",
                "3eecebfdbcab1e3c", "3fdffff276f1c000", "40641ea0f6b7c8d5",
                "3e573f675e4559bb", "be5597a69185da0c", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 0,
            "P0:0/0 K0:0/0 P1:0/0 K1:0/0", "0:0/8/298 1:0/8/298",
        ),
        "modern-720p-rotated" to ReferenceReading(
            listOf(
                "3fd9e008ae2bd201", "bfd0656f6355bcad", "4082f508a824e44c",
                "3fd31e537512ef36", "3fd90bc6fa9f7169", "401fa3c02acb15ab",
                "3f0532b87b63e680", "3f0952a53b9fed37", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
            "P0:- K0:- P1:3/0 K1:7/0", "0:1/6/234 1:2/7/286",
        ),
        "legacy-540p-tilted" to ReferenceReading(
            listOf(
                "bfd552cab4a0bdb1", "3fca44b7862fdeec", "408340dc489ba43a",
                "bfbd1d5b905b47b4", "bfd5ee9237a5b0ff", "40806d21e330ca93",
                "3f181826b18cc906", "3f1c8eca58e40851", "3ff0000000000000",
            ),
            "0000000000000000", 2, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
            "P0:- K0:- P1:8/0 K1:-", "0:3/5/188 1:3/6/255",
        ),
        "soft-480p-blur1.9" to ReferenceReading(
            listOf(
                "bfc22178f04e1e64", "bfd48033c546793b", "408066360226465e",
                "3fd12a348896bf5e", "bfc1505fb5e40897", "406287ef0fa82d6e",
                "bf1c53c29b500535", "bf165063ddb10711", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:3/0 1:4/0 2:11/0 3:3/0", "0:- 1:- 2:- 3:-", 5,
            "P0:- K0:- P1:- K1:-", "0:4/5/285 1:4/4/245",
        ),
        "small-480p" to ReferenceReading(
            listOf(
                "3fb74b5bb9090c74", "3fd5b9ca94c22997", "4058e6e53b3d28cb",
                "bfd5ba0841a8824e", "3fb7464ec09b81b9", "4076f41b31190b2d",
                "3e8145127b8058e2", "be86fc0550f28051", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 4,
            "P0:2/0 K0:- P1:1/0 K1:-", "0:4/6/294 1:4/6/294",
        ),
        "selfie-mirrored-540p" to ReferenceReading(
            listOf(
                "bfd8d0b4f8dc45ce", "3fb60e87caf54e18", "40838bb442f72eb4",
                "3fba99faafec8dd4", "3fd8200d5825dbff", "4032cf1d0b089819",
                "3e65e3b3c6ed141a", "bf031ceadf99a021", "3ff0000000000000",
            ),
            "0000000000000000", 1, true, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
            "P0:- K0:- P1:- K1:-", "0:2/5/145 1:2/4/110",
        ),
        "overexposed-540p" to ReferenceReading(
            listOf(
                "3fd64b2d160af7cf", "bfc420af24e07fb1", "4079b04de9a626ce",
                "3fc7b9f996dc010a", "3fd5d6f5b4bca05d", "401538874739992c",
                "3f01fa5e1883a6f7", "3f06441201ef7bbb", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:10/0 2:- 3:-", 3,
            "P0:- K0:- P1:12/8 K1:-", "0:2/5/207 1:2/6/250",
        ),
        "veiled-720p" to ReferenceReading(
            listOf(
                "bfc0365d22b48a29", "3fe33889d4c2bbaa", "407ed85e07c16b1d",
                "bfdf4c000f054092", "bfc0abf548cfc139", "40861c367ebb6504",
                "3f18f8eea571ebfb", "3f1d6ee482a381de", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
            "P0:6/0 K0:16/0 P1:0/0 K1:9/0", "0:2/6/283 1:2/8/298",
        ),
        "shadow-band-540p" to ReferenceReading(
            listOf(
                "3fdc6a322736a5ec", "bfafc71aac347787", "4071c8051105d191",
                "3fafee546c038a6c", "3fdc6aedff78bf88", "4025a5b107d6b00c",
                "be73b818d805ad2e", "3e9d3cb223ffb43b", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:- 0:0/0 1:15/11 2:10/0 3:11/0", "0:5/0 1:6/0 2:3/0 3:9/0", 2,
            "P0:- K0:- P1:- K1:-", "0:1/3/44 1:1/3/44",
        ),
        "hidden-corner-540p" to ReferenceReading(
            listOf(
                "bfd7b3937fd531d2", "bfbd8bac91abe4af", "40876e6e51ac7558",
                "3fc353bece274026", "bfd800c433f34e8b", "4078c99a2fe3329c",
                "3f03493960fc473f", "3f09f1db3f0bc982", "3ff0000000000000",
            ),
            "0000000000000000", 2, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
            "P0:- K0:- P1:- K1:-", "0:2/5/146 1:2/5/184",
        ),
        "cut-corner-720p" to ReferenceReading(
            listOf(
                "3fd643cc1e754c38", "bfd644e7505e01f6", "4084cceabc83ea78",
                "3fd644e395a7941d", "3fd64692a123ccf5", "405682f988221c02",
                "be7fe54208f4295c", "3e7d26eefdd5c34e", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 1,
            "P0:1/0 K0:1/0 P1:- K1:-", "0:1/7/296 1:1/4/179",
        ),
        "barrel-480p" to ReferenceReading(
            listOf(
                "3fd4be076556447d", "bfba58133b2cdc5c", "406acb20591a5813",
                "3fbe33e55b66e401", "3fd56642d3111452", "402c1a393810f225",
                "3e7009da8aaa04df", "3f0830ea4e0a30a7", "3ff0000000000000",
            ),
            "bfbeb851eb851eb8", 0, false, "D:0/0 0:0/0 1:0/0 2:1/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
            "P0:- K0:- P1:- K1:-", "0:3/4/191 1:3/5/224",
        ),
        "upright-720p" to ReferenceReading(
            listOf(
                "bf5ba008bcd3ce40", "bfe305b4351d847d", "408f2dc962b27a0c",
                "3fe4daa9ff0297f8", "bf9f9bbac0ccb7fc", "404c20ee2bf32de8",
                "3f10fbbdde986951", "3f0c268441949c72", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 1,
            "P0:0/0 K0:0/0 P1:0/0 K1:0/0", "0:1/8/298 1:1/8/298",
        ),
        "upright-legacy-720p" to ReferenceReading(
            listOf(
                "bf734d40ac63f708", "3fe61e44ff163189", "407880e9607d4b5f",
                "bfe2553b6a775243", "bf81488654367b8f", "4085a07431cc8701",
                "3f1b98d565bc7e82", "3f204092cb09d266", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:3/0 2:2/0 3:0/0", 2,
            "P0:0/0 K0:15/0 P1:0/0 K1:10/0", "0:2/8/298 1:2/8/298",
        ),
        "upright-soft-720p" to ReferenceReading(
            listOf(
                "3fb6700af200cd27", "bfe2ed6b4977dcb9", "408b60b557e2a78c",
                "3fe1b2e8d456cfc5", "3fb020bd192c8960", "40402cbf8da2dbed",
                "3e7451548694003b", "bf116f756e0f9278", "3ff0000000000000",
            ),
            "0000000000000000", 1, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
            "P0:0/0 K0:0/0 P1:2/0 K1:6/0", "0:3/8/298 1:3/6/290",
        ),
        "upright-480p" to ReferenceReading(
            listOf(
                "3f8dc022d717619e", "3fdc2b98bc136eac", "40593c7f88af9782",
                "bfda950706d8595d", "3fa1c881091abe19", "407bd46ee0463187",
                "3e65fbcc4a6e6285", "3f16281589005824", "3ff0000000000000",
            ),
            "0000000000000000", 3, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:- 1:- 2:- 3:-", 3,
            "P0:12/8 K0:- P1:0/0 K1:0/0", "0:3/5/205 1:3/8/298",
        ),
    )

    private val referenceTracks = mapOf(
        "steady-hand-540p" to ReferenceReading(
            listOf(
                "3fd3fe3727a3225f", "bfc6d6c19bf0ba9c", "407ab75dae2089a8",
                "3fc8fc47dbb4e82c", "3fd483384ee94185", "401d0353fa850028",
                "3e4e13866530f1b1", "3f0181ce3d3cf047", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:1/0 1:0/0 2:1/0 3:0/0", 2,
            "P0:- K0:- P1:- K1:-", "0:2/6/219 1:2/6/243",
        ),
        "thumb-arrives-540p" to ReferenceReading(
            listOf(
                "3fd4e787425acb92", "bfc6cf93a574b93b", "407976eddc59137b",
                "3fc9220fffe4adc3", "3fd57315c2f843cf", "401617f8a4f3f7a7",
                "3e9a526a0595628f", "3f03719795f336a1", "3ff0000000000000",
            ),
            "0000000000000000", 0, false, "D:0/0 0:0/0 1:0/0 2:0/0 3:0/0", "0:0/0 1:0/0 2:0/0 3:0/0", 2,
            "P0:- K0:- P1:- K1:-", "0:2/5/214 1:2/6/225",
        ),
    )

    /** The four-capture merge of each merge set as the Rust reference reads it (`merge_fields`). */
    private val referenceMerges = mapOf(
        "noisy-480p" to "0:5/0 1:10/0 2:- 3:11/0",
        "mixed-frames-480p" to "0:7/0 1:9/0 2:- 3:7/0",
    )

    /** The lanes a reference `ScanSession` merges from the noisy set, in the order it offers them. */
    private val referenceSessionMerges = mapOf("noisy-480p" to "013")

    private fun bits(value: Double): String = String.format("%016x", java.lang.Double.doubleToRawLongBits(value))

    private fun laneCounts(frame: PetalDecodedFrame): String =
        PetalLane.ALL.joinToString(" ") { lane ->
            "${lane.letter}:" + (frame.lane(lane)?.let { "${it.corrected}/${it.erasures}" } ?: "-")
        }

    private fun turnCounts(results: List<PetalLaneResult?>): String =
        results.withIndex().joinToString(" ") { (sector, result) ->
            "$sector:" + (result?.let { "${it.corrected}/${it.erasures}" } ?: "-")
        }

    private fun kanaCounts(results: List<PetalKanaResult?>): String =
        results.withIndex().joinToString(" ") { (index, result) ->
            "${PetalKanaLane.ALL[index].name}:" + (result?.let { "${it.result.corrected}/${it.result.erasures}" } ?: "-")
        }

    private fun bandReadings(image: PetalLuma, frame: PetalDecodedFrame): String =
        (0 until PetalLayout.KANA_BANDS).joinToString(" ") { band ->
            val reading = PetalKana.observeBand(image, frame, band)
            "$band:" + if (reading == null) {
                "-"
            } else {
                "${PetalKana.KANA_BLURS.indexOf(reading.blur)}/${reading.pilots}/${reading.tiles.count { it != null }}"
            }
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
        assertEquals(expected.kanaLanes, kanaCounts(PetalKana.readKana(image, frame, headerOf(frame))), "$name: corrected/erasures per kana lane")
        assertEquals(expected.kanaBands, bandReadings(image, frame), "$name: kana blur, pilots and tiles read per band")
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
        val stream = PetalTestSupport.loadFixture("petal_stream_v5.json").array("streams")
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
        fun checkKana(name: String, entry: JsonObject, data: PetalFrameLanes) {
            val kana = entry.array("kana_data").map { PetalTestSupport.hex(it.jsonPrimitive.content) }
            for (lane in PetalKanaLane.ALL) {
                assertContentEquals(kana[lane.ordinal], data.kana(lane), "$name ${lane.name}")
                val word = PetalKana.encodeKanaLane(lane, kana[lane.ordinal])
                assertContentEquals(kana[lane.ordinal], PetalKana.decodeKanaLaneCounted(lane, word).data)
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
            checkKana(capture.text("name"), capture, data)
        }
        // the lane data of a tracking pair is that of its second frame
        for (element in doc.array("tracks")) {
            val pair = element.jsonObject
            assertEquals(pair.number("from_frame") + 1, pair.number("to_frame"))
            val data = encoder.laneData(pair.number("to_frame").toInt())
            for ((lane, expected) in truth(pair)) assertContentEquals(expected, data.lane(lane), "${pair.text("name")} $lane")
            checkTurns(pair.text("name"), pair, data)
            checkKana(pair.text("name"), pair, data)
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
