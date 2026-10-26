package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.double
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.array
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.hexBytes
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.longs
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.number
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.obj
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.text
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.u32

/** Conformance with every section of the shared `fixtures/petal/petal_stream_v5.json`. */
class PetalStreamFixtureTest {
    private val doc: JsonObject = PetalTestSupport.loadFixture("petal_stream_v5.json")

    /** One flag per field cell, cell 0 in the most significant bit of byte 0. */
    private fun flags(bitmap: ByteArray): BooleanArray = bits(bitmap, PetalLayout.FIELD_CELLS)

    /** The first [count] flags of a bitmap, most significant bit first. */
    private fun bits(bitmap: ByteArray, count: Int): BooleanArray =
        BooleanArray(count) { i -> ((bitmap[i / 8].toInt() ushr (7 - i % 8)) and 1) == 1 }

    private fun JsonObject.real(key: String): Double = getValue(key).jsonPrimitive.double

    private fun JsonObject.hexList(key: String): List<ByteArray> = array(key).map { PetalTestSupport.hex(it.jsonPrimitive.content) }

    @Test
    fun formatAndConstantsMatch() {
        assertEquals(5L, doc.number("fixture_version"))
        assertEquals("petal-stream-v5", doc.text("format"))
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
        assertEquals(PetalTurns.TURN_POSITIONS.toLong(), constants.number("turn_positions"))
        assertEquals(PetalTurns.TURN_CARRIERS.toLong(), constants.number("turn_carriers"))
        assertEquals(PetalTurns.TURN_WORD.toLong(), constants.number("turn_word"))
        assertEquals(PetalTurns.TURN_PARITY.toLong(), constants.number("turn_parity"))
        assertEquals(PetalTurns.TURN_ATOMS.toLong(), constants.number("turn_atoms"))
        assertEquals(constants.number("turn_id_base"), u32(PetalTurns.TURN_ID_BASE))
        for ((key, value) in listOf(
            "card_height" to PetalLayout.CARD_HEIGHT,
            "square_top" to PetalLayout.SQUARE_TOP,
            "kana_tile" to PetalLayout.KANA_TILE,
            "kana_pitch" to PetalLayout.KANA_PITCH,
            "kana_glyph_box" to PetalLayout.KANA_GLYPH_BOX,
            "kana_corner" to PetalLayout.KANA_CORNER,
            "kana_gap" to PetalLayout.KANA_GAP,
            "kana_margin" to PetalLayout.KANA_MARGIN,
            "kana_finder_clearance" to PetalLayout.KANA_FINDER_CLEARANCE,
        )) {
            assertEquals(constants.real(key), value, key)
        }
        for ((key, value) in listOf(
            "kana_rows" to PetalLayout.KANA_ROWS,
            "kana_columns" to PetalLayout.KANA_COLUMNS,
            "kana_bands" to PetalLayout.KANA_BANDS,
            "kana_pilots" to PetalLayout.KANA_PILOTS,
            "kana_data_tiles" to PetalKana.KANA_DATA_TILES,
            "kana_glyph_word" to PetalKana.KANA_GLYPH_WORD,
            "kana_glyph_parity" to PetalKana.KANA_GLYPH_PARITY,
            "kana_glyph_atoms" to PetalKana.KANA_GLYPH_ATOMS,
            "kana_polarity_word" to PetalKana.KANA_POLARITY_WORD,
            "kana_polarity_parity" to PetalKana.KANA_POLARITY_PARITY,
            "kana_polarity_atoms" to PetalKana.KANA_POLARITY_ATOMS,
        )) {
            assertEquals(constants.number(key), value.toLong(), key)
        }
        assertEquals(constants.number("kana_id_base"), u32(PetalKana.KANA_ID_BASE))
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
        val orders = layout.array("carrier_orders")
        assertEquals(PetalLayout.SECTORS, orders.size)
        for (sector in 0 until PetalLayout.SECTORS) {
            assertContentEquals(
                orders[sector].longs(),
                PetalTurns.carrierOrder(sector).map { it.toLong() }.toLongArray(),
                "carrier order of sector $sector",
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
        val tiles = layout.array("kana_tiles")
        val expected = PetalLayout.kanaTiles()
        assertEquals(expected.size, tiles.size)
        for ((index, row) in tiles.withIndex()) {
            val tile = expected[index]
            val values = row.jsonArray
            assertEquals(
                listOf(tile.band.toLong(), tile.row.toLong(), tile.column.toLong()),
                values.subList(0, 3).map { it.jsonPrimitive.long },
                "tile $index",
            )
            assertEquals(values[3].jsonPrimitive.double, tile.x, "tile $index x")
            assertEquals(values[4].jsonPrimitive.double, tile.y, "tile $index y")
            assertEquals(values[5].jsonPrimitive.long, tile.pilot.toLong(), "tile $index pilot")
        }
        for ((pilot, row) in layout.array("kana_pilot_symbols").withIndex()) {
            val values = row.longs()
            assertEquals(values[0], PetalLayout.kanaPilotGlyph(pilot).toLong(), "pilot $pilot glyph")
            assertEquals(values[1] == 1L, PetalLayout.isKanaPilotLight(pilot), "pilot $pilot polarity")
        }
        val glyphs = doc.obj("glyphs")
        assertEquals(PetalGlyphs.GLYPH_CHARS, glyphs.text("chars"))
        assertEquals(PetalGlyphs.GLYPH_GRID, glyphs.real("grid"))
        assertEquals(PetalGlyphs.STROKE_WIDTH, glyphs.real("stroke_width"))
        for ((glyph, strokes) in glyphs.array("strokes").withIndex()) {
            val expectedStrokes = PetalGlyphs.strokes(glyph)
            assertEquals(expectedStrokes.size, strokes.jsonArray.size, "glyph $glyph strokes")
            for ((index, stroke) in strokes.jsonArray.withIndex()) {
                val points = stroke.jsonArray.flatMap { point -> point.jsonArray.map { it.jsonPrimitive.double } }
                assertEquals(expectedStrokes[index].toList(), points, "glyph $glyph stroke $index")
            }
        }
    }

    @Test
    fun paletteMatches() {
        val palette = doc.obj("palette")
        fun rgb(key: String): Int = palette.array(key).longs().fold(0) { acc, part -> (acc shl 8) or part.toInt() }
        assertEquals(PetalPalette.DEFAULT.background, rgb("background"))
        assertEquals(PetalPalette.DEFAULT.light, rgb("light"))
        assertEquals(PetalPalette.DEFAULT.pink, rgb("pink"))
        assertEquals(PetalPalette.DEFAULT.backdrop, rgb("backdrop"))
        assertEquals(PetalPalette.DEFAULT.ink, rgb("ink"))
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
        for (sector in 0 until PetalLayout.SECTORS) {
            assertContentEquals(whitening.hexBytes("T$sector"), PetalTurns.turnWhitening(sector), "T$sector")
        }
        for (lane in PetalKanaLane.ALL) assertContentEquals(whitening.hexBytes(lane.name), lane.whitening(), lane.name)
    }

    @Test
    fun kanaIdsAndReaderTemplatesMatch() {
        val ids = doc.obj("kana_first_ids")
        assertEquals(PetalKanaLane.ALL.map { it.name }, ids.array("lanes").map { it.jsonPrimitive.content })
        val frames = ids.array("frames").longs()
        val rows = ids.array("ids")
        assertEquals(frames.size, rows.size)
        for (index in frames.indices) {
            assertContentEquals(
                rows[index].longs(),
                PetalKanaLane.ALL.map { u32(PetalKana.kanaFirstId(frames[index].toInt(), it)) }.toLongArray(),
                "frame ${frames[index]}",
            )
        }
        val reader = doc.obj("kana_reader")
        assertEquals(PetalKana.KANA_BLURS, reader.array("blurs").map { it.jsonPrimitive.double })
        assertEquals(PetalKana.KANA_GRID.toLong(), reader.number("grid"))
        val probes = reader.array("template_probes")
        assertTrue(probes.size >= PetalKana.KANA_BLURS.size)
        for (element in probes) {
            val probe = element.jsonObject
            val blur = PetalKana.KANA_BLURS.indexOfFirst { Math.abs(it - probe.real("blur")) < 1e-12 }
            assertTrue(blur >= 0, "known blur ${probe.real("blur")}")
            val got = PetalKana.templateProbe(
                blur,
                probe.number("glyph").toInt(),
                probe.number("light") == 1L,
                probe.number("i").toInt(),
                probe.number("j").toInt(),
            )
            assertEquals(probe.real("value"), got, 1e-12, "probe $probe")
        }
        assertEquals(PetalKana.KANA_COARSE_GRID.toLong(), reader.number("coarse_grid"))
        val coarse = reader.array("coarse_probes")
        assertTrue(coarse.isNotEmpty())
        for (element in coarse) {
            val probe = element.jsonObject
            val got = PetalKana.coarseTemplateProbe(
                probe.number("glyph").toInt(),
                probe.number("light") == 1L,
                probe.number("i").toInt(),
                probe.number("j").toInt(),
            )
            assertEquals(probe.real("value"), got, 1e-12, "coarse probe $probe")
        }
    }

    @Test
    fun turnIdsAndReaderTemplatesMatch() {
        val ids = doc.obj("turn_first_ids")
        val frames = ids.array("frames").longs()
        val sectors = ids.array("sectors").longs()
        val expected = ids.array("ids").longs()
        assertEquals(frames.size, expected.size)
        for (index in frames.indices) {
            assertEquals(
                expected[index],
                u32(PetalTurns.turnFirstId(frames[index].toInt(), sectors[index].toInt())),
                "frame ${frames[index]} sector ${sectors[index]}",
            )
        }
        val reader = doc.obj("turn_reader")
        assertEquals(PetalTurns.TURN_BLURS, reader.array("blurs").map { it.jsonPrimitive.double })
        val probes = reader.array("template_probes")
        assertEquals(6 * PetalTurns.TURN_BLURS.size, probes.size)
        for (element in probes) {
            val probe = element.jsonObject
            val blur = PetalTurns.TURN_BLURS.indexOfFirst { Math.abs(it - probe.real("blur")) < 1e-12 }
            assertTrue(blur >= 0, "known blur ${probe.real("blur")}")
            val got = PetalTurns.turnTemplate(blur, probe.real("along"), probe.real("across"))
            assertEquals(probe.real("value"), got, 1e-12, "blur ${probe.real("blur")} at (${probe.real("along")}, ${probe.real("across")})")
        }
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
                val turnData = frame.hexList("turn_data")
                assertEquals(PetalLayout.SECTORS, turnData.size)
                for (sector in 0 until PetalLayout.SECTORS) {
                    assertContentEquals(turnData[sector], data.turn(sector), "$name frame $frameNumber T$sector")
                }
                val kanaData = frame.hexList("kana_data")
                val kanaWords = frame.hexList("kana_words")
                assertEquals(PetalKanaLane.ALL.size, kanaData.size)
                val kanaHeader = PetalLaneHeader(meta.tag, frameNumber)
                for (lane in PetalKanaLane.ALL) {
                    assertContentEquals(kanaData[lane.ordinal], data.kana(lane), "$name frame $frameNumber ${lane.name}")
                    assertContentEquals(
                        kanaWords[lane.ordinal],
                        PetalKana.encodeKanaLane(lane, kanaData[lane.ordinal]),
                        "$name frame $frameNumber ${lane.name} word",
                    )
                    val packet = assertNotNull(
                        PetalKana.kanaPacket(kanaHeader, lane, kanaData[lane.ordinal]),
                        "$name frame $frameNumber ${lane.name} check",
                    )
                    assertEquals(PetalKana.kanaFirstId(frameNumber, lane), packet.firstId)
                    assertEquals(lane.atoms, packet.atomCount)
                }
                val dWord = frame.hexBytes("d_word")
                val fieldWords = frame.hexList("field_words")
                val turnWords = frame.hexList("turn_words")
                assertContentEquals(dWord, PetalLanes.encodeLane(PetalLane.D, data.d()))
                val words = encoder.words(frameNumber)
                assertContentEquals(dWord, words.d())
                for (lane in PetalLane.FIELDS) {
                    val word = fieldWords[lane.sector]
                    assertContentEquals(word, PetalLanes.encodeLane(lane, data.lane(lane)), "$name frame $frameNumber $lane")
                    assertContentEquals(word, words.lane(lane), "$name frame $frameNumber $lane")
                }
                val header = PetalLaneHeader(meta.tag, frameNumber)
                for (sector in 0 until PetalLayout.SECTORS) {
                    val word = turnWords[sector]
                    assertContentEquals(word, PetalTurns.encodeTurnLane(sector, turnData[sector]), "$name frame $frameNumber turn sector $sector")
                    assertContentEquals(word, words.turn(sector), "$name frame $frameNumber turn sector $sector")
                    val packet = assertNotNull(PetalTurns.turnPacket(header, sector, turnData[sector]), "$name frame $frameNumber turn check $sector")
                    assertEquals(PetalTurns.turnFirstId(frameNumber, sector), packet.firstId)
                    assertEquals(PetalTurns.TURN_ATOMS, packet.atomCount)
                }
                val cells = PetalFrameCells.fromWords(dWord, fieldWords).withTurns(turnWords).withKana(kanaWords)
                val turns = String(CharArray(PetalLayout.FIELD_CELLS) { '0' + cells.turn(it) })
                assertEquals(frame.text("turns"), turns, "$name frame $frameNumber turns")
                val glyphText = String(CharArray(PetalLayout.KANA_TILE_COUNT) { Character.forDigit(cells.kanaGlyph(it), 16) })
                assertEquals(frame.text("kana_glyphs"), glyphText, "$name frame $frameNumber kana glyphs")
                assertContentEquals(
                    bits(frame.hexBytes("kana_light_hex"), PetalLayout.KANA_TILE_COUNT),
                    cells.kanaLight(),
                    "$name frame $frameNumber kana polarity",
                )
                for (band in 0 until PetalLayout.KANA_BANDS) {
                    assertContentEquals(kanaWords[PetalKanaLane.of(band, false).ordinal], cells.kanaPolarityWord(band))
                    assertContentEquals(kanaWords[PetalKanaLane.of(band, true).ordinal], cells.kanaGlyphWord(band))
                }
                val lit = (0 until PetalLayout.TOTAL_SLOTS).filter { cells.isDotLit(it) }.map { it.toLong() }
                assertContentEquals(frame.array("lit_dots").longs(), lit.toLongArray(), "$name frame $frameNumber dots")
                assertContentEquals(flags(frame.hexBytes("lit_petals_hex")), cells.petals(), "$name frame $frameNumber petals")
                assertEquals(cells, encoder.cells(frameNumber))
                for (sector in 0 until PetalLayout.SECTORS) assertContentEquals(turnWords[sector], cells.turnWord(sector))
            }
            // push every fixture frame through decodeLane + the assembler, without the turn and kana
            // lanes, with the turn lanes, and with both: each layer finishes the one-pass stream in
            // fewer frames
            if (name == "one-pass") {
                val needed = IntArray(3)
                for (index in 0 until 3) {
                    val withTurns = index >= 1
                    val withKana = index >= 2
                    val assembler = PetalStreamAssembler()
                    for (frameElement in stream.array("frames")) {
                        val frame = frameElement.jsonObject
                        needed[index] += 1
                        val d = PetalLanes.decodeLane(PetalLane.D, frame.hexBytes("d_word"))
                        assembler.pushDLane(assertNotNull(PetalStream.parseDLane(d)))
                        val fieldWords = frame.hexList("field_words")
                        for (lane in PetalLane.FIELDS) {
                            val data = PetalLanes.decodeLane(lane, fieldWords[lane.sector])
                            assembler.pushAtoms(assertNotNull(PetalStream.parseAtomLane(lane, data)))
                        }
                        if (withTurns) {
                            val header = PetalLaneHeader(meta.tag, frame.number("frame").toInt())
                            for ((sector, word) in frame.hexList("turn_words").withIndex()) {
                                val data = PetalTurns.decodeTurnLaneCounted(sector, word).data
                                assembler.pushAtoms(assertNotNull(PetalTurns.turnPacket(header, sector, data)))
                            }
                        }
                        if (withKana) {
                            val header = PetalLaneHeader(meta.tag, frame.number("frame").toInt())
                            for ((index2, word) in frame.hexList("kana_words").withIndex()) {
                                val lane = PetalKanaLane.ALL[index2]
                                val data = PetalKana.decodeKanaLaneCounted(lane, word).data
                                assembler.pushAtoms(assertNotNull(PetalKana.kanaPacket(header, lane, data)))
                            }
                        }
                        if (assembler.progress().complete) break
                    }
                    assertContentEquals(payload, assertNotNull(assembler.takeCompleted()).payload)
                }
                assertTrue(needed[2] < needed[1] && needed[1] < needed[0], needed.contentToString())
            }
        }
    }
}
