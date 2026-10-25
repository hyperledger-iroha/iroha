package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.array
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.hexBytes
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.longs
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.number
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.obj
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.text
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.u32

/** Conformance with every section of the shared `fixtures/petal/petal_stream_v2.json`. */
class PetalStreamFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_stream_v2.json")

    /** One flag per field cell, cell 0 in the most significant bit of byte 0. */
    private fun flags(bitmap: ByteArray): BooleanArray =
        BooleanArray(PetalLayout.FIELD_CELLS) { cell -> ((bitmap[cell / 8].toInt() ushr (7 - cell % 8)) and 1) == 1 }

    private fun JsonObject.real(key: String): Double = getValue(key).jsonPrimitive.double

    private fun JsonObject.hexList(key: String): List<ByteArray> = array(key).map { PetalTestSupport.hex(it.jsonPrimitive.content) }

    @Test
    fun formatAndConstantsMatch() {
        assertEquals(2L, doc.number("fixture_version"))
        assertEquals("petal-stream-v2", doc.text("format"))
        val constants = doc.obj("constants")
        assertEquals(PetalLayout.CANVAS, constants.number("canvas").toDouble())
        assertEquals(PetalLanes.ATOM_LEN.toLong(), constants.number("atom_len"))
        assertEquals(PetalLanes.D_WORD.toLong(), constants.number("d_word"))
        assertEquals(PetalLanes.D_PARITY.toLong(), constants.number("d_parity"))
        assertEquals(PetalLanes.FIELD_WORD.toLong(), constants.number("field_word"))
        assertEquals(PetalLanes.FIELD_PARITY.toLong(), constants.number("field_parity"))
        assertEquals(PetalLanes.FIELD_ATOMS.toLong(), constants.number("field_atoms"))
        assertEquals(PetalLayout.SECTORS.toLong(), constants.number("sectors"))
        assertEquals(PetalLanes.ATOMS_PER_FRAME.toLong(), constants.number("atoms_per_frame"))
        assertEquals(PetalLayout.FIELD_CELLS.toLong(), constants.number("field_cells"))
        assertEquals(PetalStream.BEACON_INTERVAL.toLong(), constants.number("beacon_interval"))
        assertEquals(PetalStream.FORMAT_VERSION.toLong(), constants.number("format_version"))
    }

    @Test
    fun layoutMatches() {
        val layout = doc.obj("layout")
        assertContentEquals(
            LongArray(8) {
                (if (it % 2 == 0) PetalLayout.finderCenterX(it / 2) else PetalLayout.finderCenterY(it / 2)).toLong()
            },
            layout.array("finder_centers").longs(),
        )
        assertEquals(PetalLayout.BAND_INNER, layout.real("band_inner"))
        assertEquals(PetalLayout.BAND_SPACING, layout.real("band_spacing"))
        assertContentEquals(
            LongArray(PetalLayout.BAND_COUNT) { PetalLayout.bandSlots(it).toLong() },
            layout.array("band_slots").longs(),
        )
        assertEquals(PetalLayout.PETAL_RADIUS, layout.real("petal_radius"))
        assertEquals(PetalLayout.PETAL_HALF_LENGTH, layout.real("petal_half_length"))
        assertEquals(PetalLayout.PETAL_HALF_WIDTH, layout.real("petal_half_width"))
        assertEquals(PetalLayout.PETAL_NOTCH_RADIUS, layout.real("petal_notch_radius"))
        assertEquals(PetalLayout.PETAL_NOTCH_DISTANCE, layout.real("petal_notch_distance"))
        val sectors = layout.array("sector_cells")
        assertEquals(PetalLayout.SECTORS, sectors.size)
        for (sector in 0 until PetalLayout.SECTORS) {
            assertContentEquals(
                sectors[sector].longs(),
                PetalLayout.sectorCells(sector).map { it.toLong() }.toLongArray(),
                "sector $sector",
            )
        }
        assertEquals(PetalLayout.MASK, layout.array("mask").map { it.jsonPrimitive.content })
        assertEquals(PetalLayout.MASK_ORIGIN, layout.real("mask_origin"))
        assertEquals(PetalLayout.MASK_PITCH, layout.real("mask_pitch"))
        assertEquals(PetalLayout.BACKDROP_RADIUS, layout.real("backdrop_radius"))
        assertContentEquals(
            flags(layout.hexBytes("light_cells_hex")),
            BooleanArray(PetalLayout.FIELD_CELLS) { PetalLayout.isLightCell(it) },
            "light cells",
        )
        assertContentEquals(
            LongArray(PetalLayout.RING_COUNT) { PetalLayout.ringRadius(it).toLong() },
            layout.array("ring_radii").longs(),
        )
        assertContentEquals(
            LongArray(PetalLayout.RING_COUNT) { PetalLayout.ringSlots(it).toLong() },
            layout.array("ring_slots").longs(),
        )
        assertEquals(PetalLayout.DOT_RADIUS, layout.real("dot_radius"))
        assertContentEquals(PetalLayout.dataSlots().map { it.toLong() }.toLongArray(), layout.array("data_slots").longs())
        assertContentEquals(PetalLayout.gateSlots().map { it.toLong() }.toLongArray(), layout.array("gate_slots").longs())
        assertContentEquals(PetalLayout.guardSlots().map { it.toLong() }.toLongArray(), layout.array("guard_slots").longs())
    }

    @Test
    fun paletteMatches() {
        val palette = doc.obj("palette")
        fun rgb(key: String): Int = palette.array(key).longs().fold(0) { acc, part -> (acc shl 8) or part.toInt() }
        assertEquals(PetalPalette.DEFAULT.background, rgb("background"))
        assertEquals(PetalPalette.DEFAULT.light, rgb("light"))
        assertEquals(PetalPalette.DEFAULT.pink, rgb("pink"))
        assertEquals(PetalPalette.DEFAULT.backdrop, rgb("backdrop"))
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
        assertContentEquals(whitening.hexBytes("D"), PetalLane.D.whitening())
        for (lane in PetalLane.FIELDS) assertContentEquals(whitening.hexBytes("F${lane.sector}"), lane.whitening(), "$lane")
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
                assertContentEquals(frame.hexBytes("d_data"), data.d(), "$name frame $frameNumber D")
                val fieldData = frame.hexList("field_data")
                assertEquals(PetalLayout.SECTORS, fieldData.size)
                for (sector in 0 until PetalLayout.SECTORS) {
                    assertContentEquals(fieldData[sector], data.field(sector), "$name frame $frameNumber F$sector")
                }
                val dWord = frame.hexBytes("d_word")
                val fieldWords = frame.hexList("field_words")
                assertContentEquals(dWord, PetalLanes.encodeLane(PetalLane.D, data.d()))
                val words = encoder.words(frameNumber)
                assertContentEquals(dWord, words.d())
                for (lane in PetalLane.FIELDS) {
                    val word = fieldWords[lane.sector]
                    assertContentEquals(word, PetalLanes.encodeLane(lane, data.lane(lane)), "$name frame $frameNumber $lane")
                    assertContentEquals(word, words.lane(lane), "$name frame $frameNumber $lane")
                }
                val cells = PetalFrameCells.fromWords(dWord, fieldWords)
                val lit = (0 until PetalLayout.TOTAL_SLOTS).filter { cells.isDotLit(it) }.map { it.toLong() }
                assertContentEquals(frame.array("lit_dots").longs(), lit.toLongArray(), "$name frame $frameNumber dots")
                assertContentEquals(flags(frame.hexBytes("lit_petals_hex")), cells.petals(), "$name frame $frameNumber petals")
                assertEquals(cells, encoder.cells(frameNumber))
            }
            // push every fixture frame through decodeLane + the assembler
            if (name == "one-pass") {
                val assembler = PetalStreamAssembler()
                for (frameElement in stream.array("frames")) {
                    val frame = frameElement.jsonObject
                    val d = PetalLanes.decodeLane(PetalLane.D, frame.hexBytes("d_word"))
                    assembler.pushDLane(assertNotNull(PetalStream.parseDLane(d)))
                    val fieldWords = frame.hexList("field_words")
                    for (lane in PetalLane.FIELDS) {
                        val data = PetalLanes.decodeLane(lane, fieldWords[lane.sector])
                        assembler.pushAtoms(assertNotNull(PetalStream.parseAtomLane(lane, data)))
                    }
                }
                assertContentEquals(payload, assertNotNull(assembler.takeCompleted()).payload)
            }
        }
    }
}
