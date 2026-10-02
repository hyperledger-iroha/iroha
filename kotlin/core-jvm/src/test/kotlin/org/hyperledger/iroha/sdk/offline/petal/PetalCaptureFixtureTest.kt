package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.boolean
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.array
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.hexBytes
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.number
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.text

/**
 * Decodes the golden camera captures in `fixtures/petal/petal_captures_v1.json`.
 *
 * Every conforming decoder must read the lanes named in `must_decode`, must
 * never report wrong data for any lane, and must reject the negatives. This
 * port mirrors the reference arithmetic, so it must also read exactly the
 * lanes the reference read (`reference_decoded`). Three of the nine captures
 * (`overexposed-540p`, `veiled-720p`, `shadow-band-540p`) only decode in full
 * through the normalised tile read, so they prove that it is implemented
 * faithfully.
 */
class PetalCaptureFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_captures_v1.json")

    private fun lumaOf(entry: JsonObject): PetalLuma {
        val data = PetalTestSupport.inflateBase64(entry.text("luma_zlib_base64"))
        return assertNotNull(PetalLuma.fromRaw(entry.number("width").toInt(), entry.number("height").toInt(), data))
    }

    @Test
    fun goldenCapturesDecodeAsRecorded() {
        val assembler = PetalStreamAssembler()
        val report = StringBuilder("Petal golden captures (Kotlin/JVM decode times of decodes 1, 2, 3 and the best of 6):\n")
        assertEquals(9, doc.array("captures").size)
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val image = lumaOf(capture)
            val started = System.nanoTime()
            val result = PetalDecoder.decode(image)
            val first = (System.nanoTime() - started) / 1e6
            val decoded = assertNotNull(result.frame, "$name: ${result.error}")
            assertEquals(capture.getValue("mirrored").jsonPrimitive.boolean, decoded.mirrored, "$name: mirror flag")
            val must = capture.text("must_decode")
            for ((lane, key) in listOf(PetalLane.P to "p_data", PetalLane.K to "k_data", PetalLane.D to "d_data")) {
                val laneResult = decoded.lane(lane)
                if (laneResult != null) {
                    assertContentEquals(capture.hexBytes(key), laneResult.data, "$name: lane ${lane.letter} data")
                } else {
                    assertTrue(lane.letter !in must, "$name: required lane ${lane.letter} was not decoded")
                }
            }
            assertEquals(capture.text("reference_decoded"), decoded.lanes, "$name: lanes read by the reference")
            decoded.feed(assembler)
            // timing: the JIT warms up over the first decodes
            val timings = DoubleArray(6)
            timings[0] = first
            for (run in 1 until timings.size) {
                val again = System.nanoTime()
                val repeated = PetalDecoder.decode(image)
                timings[run] = (System.nanoTime() - again) / 1e6
                assertEquals(decoded.lanes, assertNotNull(repeated.frame).lanes, "$name: decoding is deterministic")
            }
            report.append(
                String.format(
                    "  %-22s %4dx%-4d lanes %-3s (reference %-3s, required %-3s)  " +
                        "#1 %6.1f ms  #2 %6.1f ms  #3 %6.1f ms  best %6.1f ms%n",
                    name, image.width, image.height, decoded.lanes, capture.text("reference_decoded"), must,
                    timings[0], timings[1], timings[2], timings.minOrNull()!!,
                ),
            )
        }
        println(report)
        // captures of different frames of the same stream accumulate in one assembler
        assertTrue(assembler.progress().atomsReceived > 10)
    }

    /**
     * What the Rust reference read from a capture: the `decode` homography bits and orientation,
     * corrected/erasures per lane, and the tile lanes (`P`, `K`) that the level read and the
     * normalised read decode on their own under that pose.
     */
    private class ReferenceReading(
        val homographyBits: List<String>,
        val rotation: Int,
        val mirrored: Boolean,
        val lanes: String,
        val levelTileLanes: String,
        val normalisedTileLanes: String,
    )

    /**
     * Intermediates dumped from the Rust reference (`iroha_petal::decode::decode` and its tile
     * reads on these captures, macOS/arm64). The spec does not require bit-identical
     * intermediates across platforms; this port follows the reference's IEEE
     * double order of operations with deterministic `StrictMath`, and pinning the
     * values guards it against arithmetic drift.
     */
    private val referenceReadings = mapOf(
        "clean-512" to ReferenceReading(
            listOf(
                "3fe0000b84a0a4ca", "3f047a0124f40f13", "bf6709414995ad04",
                "3c829ce61c173ae9", "3fe000744f8da430", "bfa1deaf50e35321",
                "3c029daac5ea12f4", "3e847a0124f3ff14", "3ff0000000000000",
            ),
            0, false, "P:0/0 K:0/0 D:0/0", "PK", "PK",
        ),
        "modern-720p-rotated" to ReferenceReading(
            listOf(
                "3fd9dfeb4809c413", "bfd06566a21ce590", "4082f4fefe605541",
                "3fd31e4350d1fab6", "3fd90bc0e054b289", "401fa98edc6722ca",
                "3f0531825a194e74", "3f0952cfffe823bd", "3ff0000000000000",
            ),
            0, false, "P:0/0 K:0/0 D:0/0", "PK", "PK",
        ),
        "legacy-540p-tilted" to ReferenceReading(
            listOf(
                "bfd552ce1a1b1ff8", "3fca44c929f242c8", "408340e01c572c17",
                "bfbd1d3da10eaf48", "bfd5ee9c89d18c46", "40806d13824a2df7",
                "3f1817ebfd2e0ef2", "3f1c8f3483e647bf", "3ff0000000000000",
            ),
            2, false, "P:0/0 K:- D:0/0", "P", "",
        ),
        "soft-480p-blur1.9" to ReferenceReading(
            listOf(
                "bfc2215ff6ce0cfc", "bfd48045d2a13568", "4080662b2634cbbf",
                "3fd12a3a40cd55ce", "bfc1506d167c0179", "406287e0ff7c8401",
                "bf1c535776e1421a", "bf1651400c81b6b8", "3ff0000000000000",
            ),
            1, false, "P:2/0 K:- D:0/0", "P", "",
        ),
        "small-480p" to ReferenceReading(
            listOf(
                "3fb74b7c4736e5a5", "3fd5b9a944e6d7a5", "4058e74af36bae87",
                "bfd5ba00c1379c28", "3fb746496cce8cf3", "4076f4121d6166b4",
                "3e81d4a64a785c49", "be889c51d6a5b5ef", "3ff0000000000000",
            ),
            3, false, "P:0/0 K:- D:0/0", "P", "",
        ),
        "selfie-mirrored-540p" to ReferenceReading(
            listOf(
                "bfd8d0ae55b75ad1", "3fb60e52b98c3e2e", "40838bb679a30ccd",
                "3fba9a2775678d05", "3fd8200887a075e5", "4032cff70cb4d374",
                "3e6748fdc5046750", "bf031d9551fa441e", "3ff0000000000000",
            ),
            1, true, "P:0/0 K:0/0 D:0/0", "PK", "PK",
        ),
        "overexposed-540p" to ReferenceReading(
            listOf(
                "3fd64a8fb58040d1", "bfc41fa4a0c4f965", "4079b04613ed93ed",
                "3fc7b9b09d5d6f29", "3fd5d770910352b9", "40153ac08b67d7d0",
                "3f01f323d103615b", "3f064eb0d01e50fa", "3ff0000000000000",
            ),
            0, false, "P:0/0 K:11/0 D:0/0", "", "PK",
        ),
        "veiled-720p" to ReferenceReading(
            listOf(
                "bfc03614b2d9fb44", "3fe339347ddf68ef", "407ed88487f21eb9",
                "bfdf4c0483869f56", "bfc0abb61dcb3107", "40861c3f23cfa28a",
                "3f18fb7538983040", "3f1d73cd71131e4f", "3ff0000000000000",
            ),
            3, false, "P:2/0 K:4/0 D:0/0", "P", "PK",
        ),
        "shadow-band-540p" to ReferenceReading(
            listOf(
                "3fdc6a410938d22f", "bfafc5f1e8540d3a", "4071c7ee03724e99",
                "3fafeea3497b1348", "3fdc6b00dcfae1d5", "4025a94cfb231ec7",
                "be74512d487508cc", "3e9e570d4d3da448", "3ff0000000000000",
            ),
            0, false, "P:1/0 K:6/0 D:-", "", "PK",
        ),
    )

    @Test
    fun decoderArithmeticMatchesTheReferenceBitForBit() {
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val expected = referenceReadings.getValue(name)
            val decoded = assertNotNull(PetalDecoder.decode(lumaOf(capture)).frame, name)
            val bits = decoded.homography.toArray().map { String.format("%016x", java.lang.Double.doubleToRawLongBits(it)) }
            assertEquals(expected.homographyBits, bits, "$name: homography")
            assertEquals(expected.rotation, decoded.rotation, "$name: rotation")
            assertEquals(expected.mirrored, decoded.mirrored, "$name: mirrored")
            val lanes = listOf(PetalLane.P, PetalLane.K, PetalLane.D).joinToString(" ") { lane ->
                "${lane.letter}:" + (decoded.lane(lane)?.let { "${it.corrected}/${it.erasures}" } ?: "-")
            }
            assertEquals(expected.lanes, lanes, "$name: corrected/erasures per lane")
        }
    }

    /** Letters of the tile lanes (`P`, `K`) that decode from [reads] on their own. */
    private fun tileLanes(reads: PetalDecoder.TileReads): String {
        val words = PetalDecoder.tileWords(reads)
        return buildString {
            if (PetalDecoder.decodeWithErasures(PetalLane.P, words.p, words.pConfidence) != null) append('P')
            if (PetalDecoder.decodeWithErasures(PetalLane.K, words.k, words.kConfidence) != null) append('K')
        }
    }

    @Test
    fun theNormalisedReadRescuesWhatTheLevelReadLoses() {
        val options = PetalDecodeOptions.DEFAULT
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val name = capture.text("name")
            val expected = referenceReadings.getValue(name)
            val image = lumaOf(capture)
            val decoded = assertNotNull(PetalDecoder.decode(image).frame, name)
            val h = decoded.homography.m
            val reference = assertNotNull(PetalDecoder.referenceLevels(image, h), "$name: reference levels")
            val workspace = PetalWorkspace()
            PetalDecoder.samplePatches(image, h, workspace)
            val level = tileLanes(PetalDecoder.readTiles(reference, options, workspace))
            val normalised = tileLanes(PetalDecoder.readTilesNormalised(options, workspace))
            assertEquals(expected.levelTileLanes, level, "$name: lanes of the level read")
            assertEquals(expected.normalisedTileLanes, normalised, "$name: lanes of the normalised read")
            // decode takes a lane from the level read when it can and from the normalised read otherwise,
            // so it never loses a lane the level read has and gains every lane the normalised read has
            val frameTileLanes = (if (decoded.p != null) "P" else "") + (if (decoded.k != null) "K" else "")
            assertEquals(listOf('P', 'K').filter { it in level || it in normalised }.joinToString(""), frameTileLanes, name)
        }
        // the three lighting captures are the ones the level read alone cannot decode in full
        for (name in listOf("overexposed-540p", "veiled-720p", "shadow-band-540p")) {
            val reading = referenceReadings.getValue(name)
            assertTrue(reading.levelTileLanes.length < reading.normalisedTileLanes.length, "$name needs the normalised read")
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
            assertNotNull(result.error)
        }
    }

    @Test
    fun recordedLaneDataBelongsToTheOnePassStream() {
        val payload = PetalTestSupport.hex(doc.text("payload_hex"))
        val encoder = PetalStreamEncoder(payload, doc.number("payload_kind").toInt())
        val stream = PetalTestSupport.loadFixture("petal_stream_v1.json").array("streams")
            .map { it.jsonObject }.single { it.text("name") == "one-pass" }
        assertContentEquals(stream.hexBytes("payload_hex"), payload)
        for (element in doc.array("captures")) {
            val capture = element.jsonObject
            val frame = capture.number("frame").toInt()
            val data = encoder.laneData(frame)
            assertContentEquals(capture.hexBytes("p_data"), data.p())
            assertContentEquals(capture.hexBytes("k_data"), data.k())
            assertContentEquals(capture.hexBytes("d_data"), data.d())
            val word = PetalLanes.encodeLane(PetalLane.P, capture.hexBytes("p_data"))
            assertContentEquals(capture.hexBytes("p_data"), PetalLanes.decodeLane(PetalLane.P, word))
        }
    }
}
