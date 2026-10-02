package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.array
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.hexBytes
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.longs
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.number
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.obj
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.text
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.u32

/** Conformance with every section of the shared `fixtures/petal/petal_stream_v1.json`. */
class PetalStreamFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_stream_v1.json")

    @Test
    fun formatAndConstantsMatch() {
        assertEquals(1L, doc.number("fixture_version"))
        assertEquals("petal-stream-v1", doc.text("format"))
        val constants = doc.obj("constants")
        assertEquals(PetalLayout.CANVAS, constants.number("canvas").toDouble())
        assertEquals(PetalLayout.TILE_ORIGIN, constants.number("tile_origin").toDouble())
        assertEquals(PetalLayout.TILE_PITCH, constants.number("tile_pitch").toDouble())
        assertEquals(PetalLayout.TILE_SIZE, constants.number("tile_size").toDouble())
        assertEquals(PetalLayout.DOT_RADIUS, constants.number("dot_radius").toDouble())
        assertEquals(PetalLanes.ATOM_LEN.toLong(), constants.number("atom_len"))
        assertEquals(PetalLanes.P_WORD.toLong(), constants.number("p_word"))
        assertEquals(PetalLanes.K_WORD.toLong(), constants.number("k_word"))
        assertEquals(PetalLanes.D_WORD.toLong(), constants.number("d_word"))
        assertEquals(PetalLanes.P_PARITY.toLong(), constants.number("p_parity"))
        assertEquals(PetalLanes.K_PARITY.toLong(), constants.number("k_parity"))
        assertEquals(PetalLanes.D_PARITY.toLong(), constants.number("d_parity"))
        assertEquals(PetalStream.BEACON_INTERVAL.toLong(), constants.number("beacon_interval"))
        assertEquals(PetalStream.FORMAT_VERSION.toLong(), constants.number("format_version"))
    }

    @Test
    fun layoutMatches() {
        val layout = doc.obj("layout")
        assertEquals(PetalLayout.MASK, layout.array("mask").map { it.jsonPrimitive.content })
        val tiles = layout.array("tiles_col_row").longs()
        assertEquals(2 * PetalLayout.TILE_COUNT, tiles.size)
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            assertEquals(PetalLayout.tileColumn(tile).toLong(), tiles[2 * tile], "tile $tile column")
            assertEquals(PetalLayout.tileRow(tile).toLong(), tiles[2 * tile + 1], "tile $tile row")
        }
        assertContentEquals(
            LongArray(PetalLayout.RING_COUNT) { PetalLayout.ringRadius(it).toLong() },
            layout.array("ring_radii").longs(),
        )
        assertContentEquals(
            LongArray(PetalLayout.RING_COUNT) { PetalLayout.ringSlots(it).toLong() },
            layout.array("ring_slots").longs(),
        )
        assertContentEquals(
            LongArray(8) {
                (if (it % 2 == 0) PetalLayout.finderCenterX(it / 2) else PetalLayout.finderCenterY(it / 2)).toLong()
            },
            layout.array("finder_centers").longs(),
        )
        assertContentEquals(PetalLayout.dataSlots().map { it.toLong() }.toLongArray(), layout.array("data_slots").longs())
        assertContentEquals(PetalLayout.gateSlots().map { it.toLong() }.toLongArray(), layout.array("gate_slots").longs())
        assertContentEquals(PetalLayout.guardSlots().map { it.toLong() }.toLongArray(), layout.array("guard_slots").longs())
    }

    @Test
    fun glyphAlphabetStrokesAndTemplatesMatch() {
        val glyphs = doc.obj("glyphs")
        assertEquals(PetalGlyphs.GLYPH_CHARS, glyphs.text("chars"))
        assertEquals(PetalGlyphs.STROKE_WIDTH, glyphs.getValue("stroke_width").jsonPrimitive.double, 1e-9)
        val strokes = glyphs.array("strokes")
        val templates = glyphs.array("templates")
        assertEquals(PetalGlyphs.GLYPH_COUNT, strokes.size)
        assertEquals(PetalGlyphs.GLYPH_COUNT, templates.size)
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            val expected = strokes[glyph].jsonArray.map { stroke -> stroke.longs().map { it.toDouble() }.toDoubleArray() }
            val actual = PetalGlyphs.strokes(glyph)
            assertEquals(expected.size, actual.size, "glyph $glyph stroke count")
            expected.zip(actual).forEach { (want, have) -> assertContentEquals(want, have, "glyph $glyph strokes") }
            assertContentEquals(
                templates[glyph].longs().map { it.toInt() }.toIntArray(),
                PetalGlyphs.template(glyph),
                "glyph $glyph template",
            )
        }
    }

    @Test
    fun checksumsPrngAndWhiteningMatch() {
        for (case in doc.array("crc32c")) {
            val entry = case.jsonObject
            assertEquals(entry.number("crc32c"), u32(PetalCrc32c.compute(entry.hexBytes("input_hex"))))
        }
        val prng = doc.obj("prng")
        val rng = PetalXorshift32(1)
        assertContentEquals(prng.array("xorshift32_seed1").longs(), LongArray(6) { u32(rng.nextInt()) })
        for (case in prng.array("mix32")) {
            val entry = case.jsonObject
            assertEquals(entry.number("out"), u32(PetalFountain.mix32(entry.number("in").toInt())))
        }
        val whitening = doc.obj("whitening")
        assertContentEquals(whitening.hexBytes("P"), PetalLane.P.whitening())
        assertContentEquals(whitening.hexBytes("K"), PetalLane.K.whitening())
        assertContentEquals(whitening.hexBytes("D"), PetalLane.D.whitening())
    }

    @Test
    fun reedSolomonVectorsEncodeAndCorrect() {
        for (case in doc.array("reed_solomon")) {
            val entry = case.jsonObject
            val nsym = entry.number("nsym").toInt()
            val word = entry.hexBytes("codeword_hex")
            val rs = PetalReedSolomon(nsym)
            assertContentEquals(word, rs.encode(entry.hexBytes("data_hex")))
            // damage up to the correction capacity and recover
            val damaged = word.copyOf()
            for (i in 0 until nsym / 2) {
                val position = i * 3 % word.size
                damaged[position] = (damaged[position].toInt() xor 0x5A).toByte()
            }
            rs.decode(damaged)
            assertContentEquals(word, damaged)
        }
    }

    @Test
    fun fountainMasksAndAtomIdsMatch() {
        for (case in doc.array("fountain_masks")) {
            val entry = case.jsonObject
            val mask = PetalFountain.maskWords(
                entry.number("k").toInt(),
                entry.number("crc").toInt(),
                entry.number("id").toInt(),
            )
            assertContentEquals(entry.array("mask").longs(), LongArray(mask.size) { u32(mask[it]) })
        }
        val ids = doc.obj("first_atom_ids")
        val frames = ids.array("frames").longs()
        val expected = ids.array("ids").longs()
        assertEquals(frames.size, expected.size)
        for (index in frames.indices) {
            assertEquals(expected[index], PetalStream.firstAtomId(frames[index].toInt()).toLong(), "frame ${frames[index]}")
        }
    }

    @Test
    fun streamsEncodeIdenticallyAndReassemble() {
        val streams = doc.array("streams")
        assertEquals(listOf("tiny", "one-pass", "wrap"), streams.map { it.jsonObject.text("name") })
        for (element in streams) {
            val stream = element.jsonObject
            val name = stream.text("name")
            val payload = stream.hexBytes("payload_hex")
            val encoder = PetalStreamEncoder(payload, stream.number("kind").toInt())
            val meta = encoder.meta
            assertEquals(stream.number("len"), meta.length.toLong(), name)
            assertEquals(stream.number("crc32c"), u32(meta.crc), name)
            assertEquals(stream.number("tag"), meta.tag.toLong(), name)
            assertEquals(stream.number("source_atoms"), meta.sourceAtoms.toLong(), name)
            assertEquals(stream.number("systematic_frames"), encoder.systematicFrames.toLong(), name)
            for (frameElement in stream.array("frames")) {
                val frame = frameElement.jsonObject
                val frameNumber = frame.number("frame").toInt()
                val data = encoder.laneData(frameNumber)
                assertContentEquals(frame.hexBytes("p_data"), data.p(), "$name frame $frameNumber P")
                assertContentEquals(frame.hexBytes("k_data"), data.k(), "$name frame $frameNumber K")
                assertContentEquals(frame.hexBytes("d_data"), data.d(), "$name frame $frameNumber D")
                val pWord = frame.hexBytes("p_word")
                val kWord = frame.hexBytes("k_word")
                val dWord = frame.hexBytes("d_word")
                assertContentEquals(pWord, PetalLanes.encodeLane(PetalLane.P, data.p()))
                assertContentEquals(kWord, PetalLanes.encodeLane(PetalLane.K, data.k()))
                assertContentEquals(dWord, PetalLanes.encodeLane(PetalLane.D, data.d()))
                val words = encoder.words(frameNumber)
                assertContentEquals(pWord, words.p())
                assertContentEquals(kWord, words.k())
                assertContentEquals(dWord, words.d())
                val cells = PetalFrameCells.fromWords(pWord, kWord, dWord)
                assertEquals(frame.text("glyphs"), cells.glyphs().joinToString("") { Integer.toHexString(it) })
                val lit = (0 until PetalLayout.TOTAL_SLOTS).filter { cells.isDotLit(it) }.map { it.toLong() }
                assertContentEquals(frame.array("lit_dots").longs(), lit.toLongArray(), "$name frame $frameNumber dots")
                assertEquals(cells, encoder.cells(frameNumber))
            }
            // push every fixture frame through decodeLane + the assembler
            if (name == "one-pass") {
                val assembler = PetalStreamAssembler()
                for (frameElement in stream.array("frames")) {
                    val frame = frameElement.jsonObject
                    val d = PetalLanes.decodeLane(PetalLane.D, frame.hexBytes("d_word"))
                    assembler.pushDLane(assertNotNull(PetalStream.parseDLane(d)))
                    for ((lane, key) in listOf(PetalLane.P to "p_word", PetalLane.K to "k_word")) {
                        val data = PetalLanes.decodeLane(lane, frame.hexBytes(key))
                        assembler.pushAtoms(assertNotNull(PetalStream.parseAtomLane(lane, data)))
                    }
                }
                assertContentEquals(payload, assertNotNull(assembler.takeCompleted()).payload)
            }
        }
    }
}
