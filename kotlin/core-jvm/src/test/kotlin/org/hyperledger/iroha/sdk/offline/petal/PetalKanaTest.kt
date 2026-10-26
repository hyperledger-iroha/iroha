package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Ports of the reference `kana`, `glyphs` and card `layout` tests. */
class PetalKanaTest {
    @Test
    fun laneSizesAndIdsAreConsistent() {
        assertEquals(149, PetalKana.KANA_GLYPH_WORD)
        assertEquals(37, PetalKana.KANA_POLARITY_WORD)
        assertEquals(PetalLayout.KANA_TILE_COUNT, PetalLayout.kanaTiles().size)
        assertEquals(PetalKana.KANA_DATA_TILES, PetalLayout.kanaDataTiles(0).size)
        val ids = HashSet<Long>()
        for (frame in intArrayOf(0, 1, 65_535)) {
            for (lane in PetalKanaLane.ALL) {
                for (k in 0 until lane.atoms) assertTrue(ids.add(PetalTestSupport.u32(PetalKana.kanaFirstId(frame, lane) + k)))
            }
        }
        // above every turn atom id and below the u32 limit
        assertTrue(
            PetalTestSupport.u32(PetalKana.kanaFirstId(0, PetalKanaLane.K0)) >=
                PetalTestSupport.u32(PetalTurns.turnFirstId(65_535, 3)) + 4,
        )
        assertTrue(PetalTestSupport.u32(PetalKana.kanaFirstId(65_535, PetalKanaLane.P1)) < 0xFFFF_FFFFL - 16)
        assertEquals(listOf("P0", "K0", "P1", "K1"), PetalKanaLane.ALL.map { it.name })
        assertEquals(listOf(0x10, 0x11, 0x12, 0x13), PetalKanaLane.ALL.map { it.code })
    }

    @Test
    fun whiteningIsBalancedAndDistinctPerLane() {
        val all = PetalKanaLane.ALL.map { it.whitening() }
        for ((lane, whitening) in PetalKanaLane.ALL.zip(all)) {
            val ones = whitening.sumOf { Integer.bitCount(it.toInt() and 0xFF) }
            val bits = whitening.size * 8
            assertTrue(ones > bits * 35 / 100 && ones < bits * 65 / 100, "$lane")
        }
        assertFalse(all[1].copyOf(37).contentEquals(all[0]))
        assertFalse(all[1].contentEquals(all[3]))
    }

    @Test
    fun symbolsRoundtripThroughCodewords() {
        val header = PetalLaneHeader(0x5A, 77)
        val words = ArrayList<ByteArray>()
        val atoms = ArrayList<ByteArray>()
        for (lane in PetalKanaLane.ALL) {
            val laneAtoms = if (lane.isGlyph) {
                ByteArray(112) { (((it * 7) and 0xFF) xor lane.band).toByte() }
            } else {
                ByteArray(16) { (it xor lane.band).toByte() }
            }
            atoms += laneAtoms
            words += PetalKana.encodeKanaLane(lane, PetalKana.kanaData(header, lane, laneAtoms))
        }
        val base = PetalFrameCells.of(BooleanArray(PetalLayout.TOTAL_SLOTS), BooleanArray(PetalLayout.FIELD_CELLS))
        val cells = base.withKana(words)
        for (band in 0 until PetalLayout.KANA_BANDS) {
            val polarity = PetalKanaLane.of(band, false)
            val glyph = PetalKanaLane.of(band, true)
            assertContentEquals(words[polarity.ordinal], cells.kanaPolarityWord(band))
            assertContentEquals(words[glyph.ordinal], cells.kanaGlyphWord(band))
            val data = PetalKana.decodeKanaLaneCounted(glyph, words[glyph.ordinal]).data
            val packet = assertNotNull(PetalKana.kanaPacket(header, glyph, data), "check matches")
            assertEquals(PetalKana.kanaFirstId(77, glyph), packet.firstId)
            assertContentEquals(atoms[glyph.ordinal], packet.atoms().reduce { a, b -> a + b })
            // the check refuses another frame, band or kind
            assertNull(PetalKana.kanaPacket(PetalLaneHeader(0x5A, 78), glyph, data))
            assertNull(PetalKana.kanaPacket(header, PetalKanaLane.of(1 - band, true), data))
            val polarityData = PetalKana.decodeKanaLaneCounted(polarity, words[polarity.ordinal]).data
            assertContentEquals(atoms[polarity.ordinal], assertNotNull(PetalKana.kanaPacket(header, polarity, polarityData)).atom(0))
        }
        // pilots keep their symbols; the data tiles beyond the polarity codeword are light
        for ((index, tile) in PetalLayout.kanaTiles().withIndex()) {
            if (!tile.isPilot) continue
            assertEquals(PetalLayout.kanaPilotGlyph(tile.pilot), cells.kanaGlyph(index))
            assertEquals(PetalLayout.isKanaPilotLight(tile.pilot), cells.isKanaLight(index))
        }
        for (band in 0 until PetalLayout.KANA_BANDS) {
            val data = PetalLayout.kanaDataTiles(band)
            assertTrue(cells.isKanaLight(data[296]) && cells.isKanaLight(data[297]))
        }
        assertEquals(cells, PetalFrameCells.of(cells.dots(), cells.petals(), cells.turns(), cells.kanaGlyphs(), cells.kanaLight()))
    }

    @Test
    fun templatesAreUnitAndTellGlyphsApart() {
        val points = PetalKana.KANA_GRID * PetalKana.KANA_GRID
        for (blur in PetalKana.KANA_BLURS.indices) {
            for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
                for (light in listOf(true, false)) {
                    var norm = 0.0
                    for (p in 0 until points) {
                        val value = PetalKana.templateProbe(blur, glyph, light, p % PetalKana.KANA_GRID, p / PetalKana.KANA_GRID)
                        norm += value * value
                    }
                    assertEquals(1.0, norm, 1e-9, "blur $blur glyph $glyph light $light")
                }
            }
        }
        // at the sharpest blur no two light templates correlate above 0.8
        for (a in 0 until PetalGlyphs.GLYPH_COUNT) {
            for (b in a + 1 until PetalGlyphs.GLYPH_COUNT) {
                var dot = 0.0
                for (p in 0 until points) {
                    val i = p % PetalKana.KANA_GRID
                    val j = p / PetalKana.KANA_GRID
                    dot += PetalKana.templateProbe(0, a, true, i, j) * PetalKana.templateProbe(0, b, true, i, j)
                }
                assertTrue(dot < 0.8, "glyphs $a and $b: $dot")
            }
        }
    }

    @Test
    fun theCardHolds306TilesPerBandAndEightPilotsEach() {
        assertEquals(322.0, PetalLayout.KANA_BAND_HEIGHT)
        assertEquals(1668.0, PetalLayout.CARD_HEIGHT)
        val tiles = PetalLayout.kanaTiles()
        for (band in 0 until PetalLayout.KANA_BANDS) {
            val inBand = tiles.filter { it.band == band }
            assertEquals(306, inBand.size, "band $band")
            assertEquals(PetalLayout.KANA_PILOTS, inBand.count { it.isPilot })
            assertEquals(306 - PetalLayout.KANA_PILOTS, PetalLayout.kanaDataTiles(band).size)
            for ((pilot, tile) in PetalLayout.kanaPilotTiles(band).withIndex()) assertEquals(pilot, tiles[tile].pilot)
        }
        // the bands mirror each other about the square's middle
        for (tile in tiles.filter { it.band == 0 }) {
            val mirror = assertNotNull(
                tiles.firstOrNull { it.band == 1 && it.column == tile.column && it.row == PetalLayout.KANA_ROWS - 1 - tile.row },
                "mirrored tile",
            )
            assertEquals(0.0, mirror.y + PetalLayout.KANA_TILE - PetalLayout.CANVAS + tile.y, 1e-9)
            assertEquals(tile.pilot, mirror.pilot)
        }
        // every tile centre maps back to its tile, and the square holds none
        val out = DoubleArray(3)
        for ((index, tile) in tiles.withIndex()) {
            assertTrue(PetalLayout.kanaTileAt(tile.centerX, tile.centerY, out))
            assertEquals(index, out[0].toInt())
        }
        assertFalse(PetalLayout.kanaTileAt(512.0, 512.0, out))
    }

    @Test
    fun everyGlyphHasInkInsideTheDesignGridAndStaysDistinct() {
        fun coverage(glyph: Int): DoubleArray {
            val cell = PetalGlyphs.GLYPH_GRID / 8.0
            return DoubleArray(64) { i ->
                val u = (i % 8).toDouble()
                val v = (i / 8).toDouble()
                var inked = 0
                for (sy in 0 until 8) {
                    for (sx in 0 until 8) {
                        val x = (u + (sx + 0.5) / 8.0) * cell
                        val y = (v + (sy + 0.5) / 8.0) * cell
                        if (PetalGlyphs.isInked(glyph, x, y)) inked += 1
                    }
                }
                inked / 64.0
            }
        }
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            assertTrue(coverage(glyph).sum() > 6.0, "glyph $glyph")
            for (stroke in PetalGlyphs.strokes(glyph)) for (value in stroke) assertTrue(value in 2.0..30.0)
        }
        fun feature(glyph: Int): DoubleArray {
            val values = coverage(glyph)
            val mean = values.average()
            val centred = values.map { it - mean }
            val norm = Math.sqrt(centred.sumOf { it * it })
            return centred.map { it / norm }.toDoubleArray()
        }
        for (a in 0 until PetalGlyphs.GLYPH_COUNT) {
            for (b in a + 1 until PetalGlyphs.GLYPH_COUNT) {
                val dot = feature(a).zip(feature(b)).sumOf { (x, y) -> x * y }
                assertTrue(1.0 - dot > 0.2, "glyphs $a and $b: ${1.0 - dot}")
            }
        }
    }

    @Test
    fun everyGlyphStrokeLiesInsideItsRoundedTile() {
        // what lets a native canvas stroke the glyphs without clipping them to the tile
        val inset = (PetalLayout.KANA_TILE - PetalLayout.KANA_GLYPH_BOX) / 2.0
        val scale = PetalGlyphs.GLYPH_GRID / PetalLayout.KANA_GLYPH_BOX
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            var u = -1.0
            while (u <= 26.0) {
                var v = -1.0
                while (v <= 26.0) {
                    if (PetalGlyphs.isInked(glyph, (u - inset) * scale, (v - inset) * scale)) {
                        assertTrue(PetalLayout.kanaInside(u, v), "glyph $glyph inked outside its tile at ($u, $v)")
                    }
                    v += 0.05
                }
                u += 0.05
            }
        }
        assertEquals(PetalGlyphs.STROKE_WIDTH * PetalLayout.KANA_GLYPH_BOX / PetalGlyphs.GLYPH_GRID, PetalDrawList.KANA_STROKE_WIDTH)
        assertEquals(inset + 22.0 / scale, PetalDrawList.kanaGlyphStrokes(0)[0][0])
    }
}
