package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.Lcg

/** Ports of the reference `crc`, `prng`, `rs`, `lanes`, `layout` and `glyphs` unit tests. */
class PetalCodecTest {
    @Test
    fun crc32cMatchesThePublishedCheckValue() {
        assertEquals(0xE306_9283.toInt(), PetalCrc32c.compute("123456789".toByteArray(Charsets.US_ASCII)))
        assertEquals(0, PetalCrc32c.compute(ByteArray(0)))
        val bytes = "xx123456789yy".toByteArray(Charsets.US_ASCII)
        assertEquals(0xE306_9283.toInt(), PetalCrc32c.compute(bytes, 2, 9))
        assertFailsWith<IllegalArgumentException> { PetalCrc32c.compute(bytes, 10, 9) }
    }

    @Test
    fun xorshiftMatchesTheReferenceSequence() {
        val rng = PetalXorshift32(1)
        assertEquals(270_369L, PetalTestSupport.u32(rng.nextInt()))
        assertEquals(67_634_689L, PetalTestSupport.u32(rng.nextInt()))
        assertEquals(2_647_435_461L, PetalTestSupport.u32(rng.nextInt()))
        assertEquals(PetalXorshift32(0xDEAD_BEEF.toInt()), PetalXorshift32(0))
        assertEquals(PetalXorshift32(5).nextInt() ushr 24, PetalXorshift32(5).nextByte())
    }

    @Test
    fun reedSolomonMatchesTheQrHelloWorldCheckVector() {
        // QR Code version 1-M "HELLO WORLD": 16 data codewords, 10 EC codewords.
        val data = byteArrayOf(32, 91, 11, 120, 209.toByte(), 114, 220.toByte(), 77, 67, 64, 236.toByte(), 17, 236.toByte(), 17, 236.toByte(), 17)
        val expected = intArrayOf(196, 35, 39, 119, 235, 215, 231, 226, 93, 23)
        val word = PetalReedSolomon(10).encode(data)
        assertContentEquals(data, word.copyOf(16))
        assertContentEquals(expected, IntArray(10) { word[16 + it].toInt() and 0xFF })
    }

    @Test
    fun reedSolomonCorrectsRandomErrorsAndErasuresUpToCapacity() {
        val rng = Lcg(7)
        for ((k, nsym) in listOf(16 to 16, 60 to 68, 12 to 18, 13 to 115)) {
            val rs = PetalReedSolomon(nsym)
            repeat(60) {
                val data = ByteArray(k) { rng.byte().toByte() }
                val clean = rs.encode(data)
                val n = clean.size
                // pick f erasures and e errors with 2e + f <= nsym
                val f = rng.below(minOf(nsym, n - 1) + 1)
                val e = rng.below((nsym - f) / 2 + 1)
                val word = clean.copyOf()
                val positions = IntArray(n) { it }
                for (i in 0 until f + e) {
                    val j = i + rng.below(n - i)
                    val swap = positions[i]
                    positions[i] = positions[j]
                    positions[j] = swap
                }
                val erased = positions.copyOfRange(0, f)
                for (p in erased) word[p] = rng.byte().toByte()
                for (index in f until f + e) {
                    val p = positions[index]
                    word[p] = (word[p].toInt() xor (rng.byte() or 1)).toByte()
                }
                val corrected = rs.decode(word, erased)
                assertTrue(corrected <= f + e)
                assertContentEquals(clean, word, "k=$k nsym=$nsym f=$f e=$e")
            }
        }
    }

    @Test
    fun reedSolomonRejectsWordsBeyondCapacityWithoutReturningWrongData() {
        val rng = Lcg(99)
        val rs = PetalReedSolomon(16)
        var wrongAccepts = 0
        repeat(200) {
            val data = ByteArray(16) { rng.byte().toByte() }
            val clean = rs.encode(data)
            val word = clean.copyOf()
            // 20 random errors is far beyond t = 8
            val positions = IntArray(word.size) { it }
            for (i in 0 until 20) {
                val j = i + rng.below(word.size - i)
                val swap = positions[i]
                positions[i] = positions[j]
                positions[j] = swap
            }
            for (index in 0 until 20) {
                val p = positions[index]
                word[p] = (word[p].toInt() xor (rng.byte() or 1)).toByte()
            }
            val before = word.copyOf()
            val accepted = try {
                rs.decode(word)
                true
            } catch (failure: PetalReedSolomonException) {
                assertEquals(PetalReedSolomonException.Reason.UNCORRECTABLE, failure.reason)
                assertContentEquals(before, word, "a failed decode must leave the word untouched")
                false
            }
            if (accepted) {
                // a miscorrection must at least be a valid codeword
                assertTrue(rs.syndromes(IntArray(word.size) { word[it].toInt() and 0xFF }).all { it == 0 })
                if (!word.contentEquals(clean)) wrongAccepts += 1
            }
        }
        assertTrue(wrongAccepts <= 2, "miscorrection rate too high: $wrongAccepts")
    }

    @Test
    fun reedSolomonRejectsMalformedArguments() {
        val rs = PetalReedSolomon(4)
        val short = ByteArray(4)
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { rs.decode(short) }.reason,
        )
        val word = rs.encode(byteArrayOf(1, 2, 3))
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { rs.decode(word, intArrayOf(9)) }.reason,
        )
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { rs.decode(word, intArrayOf(1, 1)) }.reason,
        )
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { rs.decode(word, intArrayOf(-1)) }.reason,
        )
        assertFailsWith<IllegalArgumentException> { PetalReedSolomon(0) }
        assertFailsWith<IllegalArgumentException> { PetalReedSolomon(255) }
        assertFailsWith<IllegalArgumentException> { PetalReedSolomon(10).encode(ByteArray(246)) }
        assertEquals(0, rs.decode(word))
    }

    @Test
    fun laneSizesAreConsistent() {
        assertEquals(32, PetalLanes.P_WORD)
        assertEquals(128, PetalLanes.K_WORD)
        assertEquals(30, PetalLanes.D_WORD)
        assertEquals(listOf(19, 83, 19), listOf(PetalLanes.P_DATA, PetalLanes.K_DATA, PetalLanes.D_DATA))
        assertEquals(listOf(19, 83, 19), PetalLane.values().map { it.dataLength }.let { listOf(it[0], it[1], it[2]) })
        assertEquals(listOf(PetalLane.P, PetalLane.D, PetalLane.K), PetalLane.DECODE_ORDER)
    }

    @Test
    fun whiteningIsDeterministicAndBalanced() {
        for (lane in PetalLane.values()) {
            val a = lane.whitening()
            assertContentEquals(a, lane.whitening())
            val ones = a.sumOf { Integer.bitCount(it.toInt() and 0xFF) }
            val bits = a.size * 8
            assertTrue(ones > bits * 38 / 100 && ones < bits * 62 / 100, "$lane ones $ones/$bits")
            a[0] = (a[0].toInt() xor 1).toByte()
            assertNotEquals(a[0], lane.whitening()[0], "whitening must be returned as a copy")
        }
    }

    @Test
    fun lanesRoundtripThroughCells() {
        val pData = ByteArray(PetalLanes.P_DATA) { it.toByte() }
        val kData = ByteArray(PetalLanes.K_DATA) { (it * 37).toByte() }
        val dData = ByteArray(PetalLanes.D_DATA) { (it xor 0xA5).toByte() }
        val p = PetalLanes.encodeLane(PetalLane.P, pData)
        val k = PetalLanes.encodeLane(PetalLane.K, kData)
        val d = PetalLanes.encodeLane(PetalLane.D, dData)
        val cells = PetalFrameCells.fromWords(p, k, d)
        assertContentEquals(p, cells.pWord())
        assertContentEquals(k, cells.kWord())
        assertContentEquals(d, cells.dWord())
        assertContentEquals(pData, PetalLanes.decodeLane(PetalLane.P, cells.pWord()))
        assertContentEquals(kData, PetalLanes.decodeLane(PetalLane.K, cells.kWord()))
        assertContentEquals(dData, PetalLanes.decodeLane(PetalLane.D, cells.dWord()))
        assertEquals(cells, PetalFrameCells.of(cells.light(), cells.glyphs(), cells.dots()))
        assertFailsWith<IllegalArgumentException> { PetalLanes.encodeLane(PetalLane.P, ByteArray(18)) }
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { PetalLanes.decodeLane(PetalLane.D, ByteArray(29)) }.reason,
        )
    }

    @Test
    fun allZeroDataStillLightsRoughlyHalfTheCells() {
        val cells = PetalFrameCells.fromWords(
            PetalLanes.encodeLane(PetalLane.P, ByteArray(PetalLanes.P_DATA)),
            PetalLanes.encodeLane(PetalLane.K, ByteArray(PetalLanes.K_DATA)),
            PetalLanes.encodeLane(PetalLane.D, ByteArray(PetalLanes.D_DATA)),
        )
        val lit = cells.light().count { it }
        assertTrue(lit in 90..166, "$lit light tiles")
    }

    @Test
    fun gateDotsAreAlwaysLitAndGuardsDark() {
        val cells = PetalFrameCells.fromWords(
            ByteArray(PetalLanes.P_WORD) { -1 },
            ByteArray(PetalLanes.K_WORD) { -1 },
            ByteArray(PetalLanes.D_WORD) { -1 },
        )
        PetalLayout.slotRoles().forEachIndexed { slot, role ->
            when (role) {
                PetalSlotRole.GATE, PetalSlotRole.DATA -> assertTrue(cells.isDotLit(slot))
                PetalSlotRole.GUARD, PetalSlotRole.SPARE -> assertTrue(!cells.isDotLit(slot))
            }
        }
    }

    @Test
    fun countedDecodingReportsTheRewrittenPositions() {
        val data = ByteArray(PetalLanes.P_DATA) { it.toByte() }
        val clean = PetalLanes.encodeLane(PetalLane.P, data)
        val exact = PetalLanes.decodeLaneCounted(PetalLane.P, clean)
        assertContentEquals(data, exact.data)
        assertEquals(0, exact.corrected)
        assertEquals(0, exact.erasures)
        val damaged = clean.copyOf()
        for (position in intArrayOf(0, 7, 19, 31)) damaged[position] = (damaged[position].toInt() xor 0xC3).toByte()
        val repaired = PetalLanes.decodeLaneCounted(PetalLane.P, damaged)
        assertContentEquals(data, repaired.data)
        assertEquals(4, repaired.corrected)
        // two of the damaged bytes flagged as erasures: still four rewritten positions
        val flagged = PetalLanes.decodeLaneCounted(PetalLane.P, damaged, intArrayOf(7, 19))
        assertContentEquals(data, flagged.data)
        assertEquals(4, flagged.corrected)
        assertEquals(2, flagged.erasures)
    }

    @Test
    fun decodeSurvivesBurstDamageToALane() {
        val kData = ByteArray(PetalLanes.K_DATA) { it.toByte() }
        val word = PetalLanes.encodeLane(PetalLane.K, kData)
        for (index in 0 until PetalLanes.K_PARITY / 2) word[index] = (word[index].toInt() xor 0x5A).toByte()
        assertContentEquals(kData, PetalLanes.decodeLane(PetalLane.K, word))
    }

    @Test
    fun maskHas256SymmetricTiles() {
        for (row in PetalLayout.MASK) {
            assertEquals(PetalLayout.TILE_GRID, row.length)
            for (column in 0 until PetalLayout.TILE_GRID / 2) {
                assertEquals(row[column], row[PetalLayout.TILE_GRID - 1 - column], "row $row not mirrored")
            }
        }
        assertNotEquals(PetalLayout.MASK[0], PetalLayout.MASK[PetalLayout.TILE_GRID - 1], "mask must show top from bottom")
        assertEquals(PetalLayout.TILE_COUNT, PetalLayout.MASK.sumOf { line -> line.count { it == '#' } })
    }

    @Test
    fun tilesAreRowMajorAndInsideTheCanvas() {
        for (index in 0 until PetalLayout.TILE_COUNT) {
            if (index > 0) {
                val previous = PetalLayout.tileRow(index - 1) * 100 + PetalLayout.tileColumn(index - 1)
                assertTrue(PetalLayout.tileRow(index) * 100 + PetalLayout.tileColumn(index) > previous)
            }
            val (x, y) = PetalLayout.tileCenter(index).let { it[0] to it[1] }
            assertTrue(x in 0.0..PetalLayout.CANVAS && y in 0.0..PetalLayout.CANVAS)
        }
    }

    @Test
    fun ringSlotsProvideExactlyTheLaneDCapacity() {
        val roles = PetalLayout.slotRoles()
        assertEquals(PetalLayout.TOTAL_SLOTS, roles.size)
        assertEquals(PetalLayout.D_BITS, roles.count { it == PetalSlotRole.DATA })
        assertEquals(4 + 5 + 7, roles.count { it == PetalSlotRole.GATE })
        assertEquals(PetalLayout.D_BITS, PetalLayout.dataSlots().size)
        // the two spare slots are the last non-reserved slots of the outer ring
        assertEquals(2, roles.count { it == PetalSlotRole.SPARE })
        PetalLayout.dataSlots().forEachIndexed { bit, slot -> assertEquals(bit, PetalLayout.slotDataBit(slot)) }
        assertEquals(-1, PetalLayout.slotDataBit(PetalLayout.gateSlots()[0]))
    }

    @Test
    fun gatesNeverTouchTheTop() {
        for (flat in PetalLayout.gateSlots() + PetalLayout.guardSlots()) {
            val (ring, slot) = PetalLayout.splitSlot(flat).let { it[0] to it[1] }
            val top = 3 * PetalLayout.ringSlots(ring) / 4
            assertTrue(Math.abs(slot - top) > 2, "ring $ring slot $slot near the top")
        }
    }

    @Test
    fun findersAndRingsDoNotOverlap() {
        val outermost = PetalLayout.ringRadius(2) + PetalLayout.DOT_RADIUS
        for (finder in 0 until 4) {
            val dx = PetalLayout.finderCenterX(finder) - PetalLayout.CENTER
            val dy = PetalLayout.finderCenterY(finder) - PetalLayout.CENTER
            assertTrue(Math.sqrt(dx * dx + dy * dy) - PetalLayout.FINDER_OUTER > outermost + 20.0)
        }
        var farthest = 0.0
        val h = PetalLayout.TILE_SIZE / 2.0
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val center = PetalLayout.tileCenter(tile)
            for (sx in listOf(-h, h)) {
                for (sy in listOf(-h, h)) {
                    val dx = center[0] + sx - PetalLayout.CENTER
                    val dy = center[1] + sy - PetalLayout.CENTER
                    farthest = maxOf(farthest, Math.sqrt(dx * dx + dy * dy))
                }
            }
        }
        assertTrue(farthest + 10.0 < PetalLayout.ringRadius(0) - PetalLayout.DOT_RADIUS)
    }

    @Test
    fun slotCentresFollowTheRingsClockwiseFromThreeOClock() {
        for (ring in 0 until PetalLayout.RING_COUNT) {
            val first = PetalLayout.slotCenter(ring, 0)
            assertEquals(PetalLayout.CENTER + PetalLayout.ringRadius(ring), first[0], 1e-3)
            assertEquals(PetalLayout.CENTER, first[1], 1e-3)
            val quarter = PetalLayout.slotCenter(ring, PetalLayout.ringSlots(ring) / 4)
            assertEquals(PetalLayout.CENTER + PetalLayout.ringRadius(ring), quarter[1], 1e-3, "slots advance clockwise")
        }
        assertTrue(PetalLayout.finderLit(0.0, 0.0))
        assertTrue(PetalLayout.finderLit(0.0, -34.0), "upper petal")
        assertTrue(!PetalLayout.finderLit(0.0, -59.0), "notch at the upper petal tip")
        assertTrue(!PetalLayout.finderLit(0.0, 59.0), "gap between the lower petals")
    }

    @Test
    fun checkedInTemplatesMatchTheStrokeDefinitions() {
        val generated = PetalGlyphs.generateTemplates()
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            assertContentEquals(PetalGlyphs.template(glyph), generated[glyph], "glyph $glyph")
        }
    }

    @Test
    fun everyGlyphHasInkInsideTheDesignGrid() {
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            assertTrue(PetalGlyphs.template(glyph).sum() > 255 * 6, "glyph $glyph has too little ink")
            for (stroke in PetalGlyphs.strokes(glyph)) {
                for (value in stroke) assertTrue(value in 2.0..30.0)
            }
        }
        assertEquals(PetalGlyphs.GLYPH_COUNT, PetalGlyphs.GLYPH_CHARS.length)
    }

    @Test
    fun glyphsArePairwiseDistinctUnderBlur() {
        // Zero-mean cosine distance of the raw templates must stay well apart.
        val features = Array(PetalGlyphs.GLYPH_COUNT) { glyph ->
            val values = PetalGlyphs.template(glyph).map { it.toDouble() }
            val mean = values.sum() / values.size
            val centered = values.map { it - mean }
            val norm = Math.sqrt(centered.sumOf { it * it })
            centered.map { it / norm }
        }
        for (a in 0 until PetalGlyphs.GLYPH_COUNT) {
            for (b in a + 1 until PetalGlyphs.GLYPH_COUNT) {
                val dot = features[a].zip(features[b]).sumOf { (x, y) -> x * y }
                assertTrue(1.0 - dot > 0.2, "glyphs $a and $b are too similar: ${1.0 - dot}")
            }
        }
    }
}
