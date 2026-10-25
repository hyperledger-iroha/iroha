package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.offline.petal.PetalTestSupport.Lcg

/** Ports of the reference `crc`, `prng`, `rs`, `lanes` and `layout` unit tests. */
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

    private fun fieldData(seed: Int): ByteArray = ByteArray(PetalLanes.FIELD_DATA) { ((it * 37) xor seed).toByte() }

    private fun fieldWords(data: (Int) -> ByteArray): List<ByteArray> =
        List(PetalLayout.SECTORS) { PetalLanes.encodeLane(PetalLane.field(it), data(it)) }

    @Test
    fun laneSizesAreConsistent() {
        assertEquals(30, PetalLanes.D_WORD)
        assertEquals(listOf(19, 83), listOf(PetalLanes.D_DATA, PetalLanes.FIELD_DATA))
        assertEquals(4, PetalLanes.SPARE_SECTOR_CELLS)
        assertEquals(21, PetalLanes.ATOMS_PER_FRAME)
        assertEquals(listOf(19, 83, 83, 83, 83), PetalLane.ALL.map { it.dataLength })
        assertEquals(listOf(PetalLane.D, PetalLane.F0, PetalLane.F1, PetalLane.F2, PetalLane.F3), PetalLane.ALL)
        assertEquals("D0123", PetalLane.ALL.joinToString("") { it.letter.toString() })
        assertEquals(PetalLane.ALL.drop(1), PetalLane.FIELDS)
        PetalLane.FIELDS.forEachIndexed { sector, lane ->
            assertEquals(sector, lane.sector)
            assertEquals(lane, PetalLane.field(sector))
            assertTrue(lane.isField)
        }
        assertEquals(-1, PetalLane.D.sector)
        assertFailsWith<IllegalArgumentException> { PetalLane.field(4) }
    }

    @Test
    fun whiteningIsDeterministicAndBalanced() {
        for (lane in PetalLane.ALL) {
            val a = lane.whitening()
            assertContentEquals(a, lane.whitening())
            val ones = a.sumOf { Integer.bitCount(it.toInt() and 0xFF) }
            val bits = a.size * 8
            assertTrue(ones > bits * 38 / 100 && ones < bits * 62 / 100, "$lane ones $ones/$bits")
            a[0] = (a[0].toInt() xor 1).toByte()
            assertNotEquals(a[0], lane.whitening()[0], "whitening must be returned as a copy")
        }
        assertTrue(!PetalLane.F0.whitening().contentEquals(PetalLane.F1.whitening()))
    }

    @Test
    fun lanesRoundtripThroughCells() {
        val dData = ByteArray(PetalLanes.D_DATA) { (it xor 0xA5).toByte() }
        val d = PetalLanes.encodeLane(PetalLane.D, dData)
        val fields = fieldWords(::fieldData)
        val cells = PetalFrameCells.fromWords(d, fields)
        assertContentEquals(d, cells.dWord())
        assertContentEquals(dData, PetalLanes.decodeLane(PetalLane.D, cells.dWord()))
        for (sector in 0 until PetalLayout.SECTORS) {
            assertContentEquals(fields[sector], cells.fieldWord(sector))
            assertContentEquals(fieldData(sector), PetalLanes.decodeLane(PetalLane.field(sector), cells.fieldWord(sector)))
        }
        // the spare cells at the end of every sector stay dark
        for (sector in 0 until PetalLayout.SECTORS) {
            val order = PetalLayout.sectorCells(sector)
            for (index in PetalLanes.FIELD_WORD * 8 until order.size) assertTrue(!cells.isPetalLit(order[index]))
        }
        assertEquals(cells, PetalFrameCells.of(cells.dots(), cells.petals()))
        assertFailsWith<IllegalArgumentException> { PetalLanes.encodeLane(PetalLane.F2, ByteArray(82)) }
        assertFailsWith<IllegalArgumentException> { PetalFrameCells.fromWords(d, fields.take(3)) }
        assertFailsWith<IllegalArgumentException> { PetalFrameCells.of(cells.dots(), BooleanArray(10)) }
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { PetalLanes.decodeLane(PetalLane.D, ByteArray(29)) }.reason,
        )
    }

    @Test
    fun allZeroDataStillLightsAboutHalfThePetals() {
        val cells = PetalFrameCells.fromWords(
            PetalLanes.encodeLane(PetalLane.D, ByteArray(PetalLanes.D_DATA)),
            fieldWords { ByteArray(PetalLanes.FIELD_DATA) },
        )
        val lit = cells.petals().count { it }
        assertTrue(lit in 1300..2050, "$lit lit petals")
    }

    @Test
    fun gateDotsAreAlwaysLitAndGuardsDark() {
        val field = ByteArray(PetalLanes.FIELD_WORD) { -1 }
        val cells = PetalFrameCells.fromWords(ByteArray(PetalLanes.D_WORD) { -1 }, List(PetalLayout.SECTORS) { field })
        PetalLayout.slotRoles().forEachIndexed { slot, role ->
            when (role) {
                PetalSlotRole.GATE, PetalSlotRole.DATA -> assertTrue(cells.isDotLit(slot))
                PetalSlotRole.GUARD, PetalSlotRole.SPARE -> assertTrue(!cells.isDotLit(slot))
            }
        }
    }

    @Test
    fun countedDecodingReportsTheRewrittenPositions() {
        val data = fieldData(9)
        val clean = PetalLanes.encodeLane(PetalLane.F2, data)
        val exact = PetalLanes.decodeLaneCounted(PetalLane.F2, clean)
        assertContentEquals(data, exact.data)
        assertEquals(0, exact.corrected)
        assertEquals(0, exact.erasures)
        val damaged = clean.copyOf()
        for (position in intArrayOf(0, 7, 19, 31, 60, 104)) damaged[position] = (damaged[position].toInt() xor 0xC3).toByte()
        val repaired = PetalLanes.decodeLaneCounted(PetalLane.F2, damaged)
        assertContentEquals(data, repaired.data)
        assertEquals(6, repaired.corrected)
        // two of the damaged bytes flagged as erasures: still six rewritten positions
        val flagged = PetalLanes.decodeLaneCounted(PetalLane.F2, damaged, intArrayOf(7, 19))
        assertContentEquals(data, flagged.data)
        assertEquals(6, flagged.corrected)
        assertEquals(2, flagged.erasures)
    }

    @Test
    fun aWordDecodedUnderAnotherSectorIsRefused() {
        val word = PetalLanes.encodeLane(PetalLane.F0, fieldData(1))
        for (lane in listOf(PetalLane.F1, PetalLane.F2, PetalLane.F3)) {
            assertEquals(
                PetalReedSolomonException.Reason.UNCORRECTABLE,
                assertFailsWith<PetalReedSolomonException> { PetalLanes.decodeLane(lane, word) }.reason,
            )
        }
    }

    @Test
    fun decodeSurvivesBurstDamageToALane() {
        val data = fieldData(3)
        val word = PetalLanes.encodeLane(PetalLane.F1, data)
        for (index in 0 until PetalLanes.FIELD_PARITY / 2) word[index] = (word[index].toInt() xor 0x5A).toByte()
        assertContentEquals(data, PetalLanes.decodeLane(PetalLane.F1, word))
    }

    @Test
    fun maskIsMirrorSymmetricAndShowsWhichWayIsUp() {
        for (row in PetalLayout.MASK) {
            assertEquals(PetalLayout.MASK_GRID, row.length)
            for (column in 0 until PetalLayout.MASK_GRID / 2) {
                assertEquals(row[column], row[PetalLayout.MASK_GRID - 1 - column], "row $row not mirrored")
            }
        }
        assertNotEquals(PetalLayout.MASK[0], PetalLayout.MASK[PetalLayout.MASK_GRID - 1], "mask must show top from bottom")
    }

    @Test
    fun bandsHoldEveryCellAndSplitIntoEqualSectors() {
        assertEquals(PetalLayout.FIELD_CELLS, PetalLayout.bandOffset(PetalLayout.BAND_COUNT))
        val seen = BooleanArray(PetalLayout.FIELD_CELLS)
        for (sector in 0 until PetalLayout.SECTORS) {
            val cells = PetalLayout.sectorCells(sector)
            assertEquals(PetalLayout.SECTOR_CELLS, cells.size)
            for (cell in cells) {
                assertTrue(!seen[cell], "cell $cell in two sectors")
                seen[cell] = true
                // a cell sits in its own quarter turn (odd bands lean half a slot forward)
                val band = PetalLayout.cellBand(cell)
                val quarter = PetalLayout.bandSlots(band) / PetalLayout.SECTORS
                assertEquals(sector, PetalLayout.cellSlot(cell) / quarter)
                val turn = PetalLayout.cellAngle(cell) / PetalLayout.TAU * PetalLayout.SECTORS
                assertTrue(turn > sector - 1e-9 && turn < sector + 1.0)
            }
        }
        assertTrue(seen.all { it })
        assertFailsWith<IllegalArgumentException> { PetalLayout.sectorCells(4) }
    }

    @Test
    fun bandSlotsFollowThePitchRule() {
        for (band in 0 until PetalLayout.BAND_COUNT) {
            val raw = PetalLayout.TAU * PetalLayout.bandRadius(band) / 50.0
            assertEquals(4 * maxOf(Math.round(raw).toInt(), 1), PetalLayout.bandSlots(band), "band $band")
            assertEquals(16.0 + 11.0 * band, PetalLayout.bandRadius(band))
            assertEquals(if (band % 2 == 1) 0.5 else 0.0, PetalLayout.bandPhase(band))
        }
    }

    @Test
    fun theFieldMapsOntoItselfUnderAQuarterTurn() {
        val c = PetalLayout.CENTER
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val band = PetalLayout.cellBand(cell)
            val n = PetalLayout.bandSlots(band)
            val turned = PetalLayout.bandOffset(band) + (PetalLayout.cellSlot(cell) + n / 4) % n
            val (x, y) = PetalLayout.cellCenter(cell).let { it[0] to it[1] }
            val (tx, ty) = PetalLayout.cellCenter(turned).let { it[0] to it[1] }
            // a quarter turn clockwise on screen about the centre
            assertTrue(Math.abs(tx - (c - (y - c))) < 1e-9 && Math.abs(ty - (c + (x - c))) < 1e-9, "cell $cell")
        }
    }

    @Test
    fun petalsStayClearOfTheirNeighboursAndTheRings() {
        val outer = PetalLayout.bandRadius(PetalLayout.BAND_COUNT - 1) + PetalLayout.PETAL_HALF_LENGTH
        assertTrue(outer + 10.0 < PetalLayout.ringRadius(0) - PetalLayout.DOT_RADIUS)
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val center = PetalLayout.cellCenter(cell)
            val band = PetalLayout.cellBand(cell)
            val near = PetalLayout.bandOffset(maxOf(band - 1, 0)) until PetalLayout.bandOffset(minOf(band + 2, PetalLayout.BAND_COUNT))
            for (other in near) {
                if (other == cell) continue
                val neighbour = PetalLayout.cellCenter(other)
                val distance = Math.hypot(center[0] - neighbour[0], center[1] - neighbour[1])
                assertTrue(distance > 2.0 * PetalLayout.PETAL_RADIUS, "cells $cell and $other are $distance apart")
            }
        }
    }

    @Test
    fun theLightCellsDrawTheCharacter() {
        val light = (0 until PetalLayout.FIELD_CELLS).count { PetalLayout.isLightCell(it) }
        assertTrue(light in 1200 until 2400, "$light light cells")
        assertTrue(PetalLayout.inMask(512.0, 190.0), "the top bar")
        assertTrue(!PetalLayout.inMask(512.0, 350.0), "the gap under the second bar")
        assertTrue(!PetalLayout.inMask(181.9, 400.0) && !PetalLayout.inMask(900.0, 900.0))
    }

    @Test
    fun ringSlotsProvideExactlyTheLaneDCapacity() {
        val roles = PetalLayout.slotRoles()
        assertEquals(PetalLayout.TOTAL_SLOTS, roles.size)
        assertEquals(PetalLayout.D_BITS, roles.count { it == PetalSlotRole.DATA })
        assertEquals(4 + 5 + 7, roles.count { it == PetalSlotRole.GATE })
        assertEquals(PetalLayout.D_BITS, PetalLayout.dataSlots().size)
        assertEquals(PetalLayout.TOTAL_SLOTS - PetalLayout.D_BITS - 16 - 18, roles.count { it == PetalSlotRole.SPARE })
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
    fun aPetalIsLitAtItsCentreAndNotchedAtItsTip() {
        assertTrue(PetalLayout.petalLit(0.0, 0.0, 0.3))
        val tip = PetalLayout.PETAL_NOTCH_DISTANCE
        assertTrue(!PetalLayout.petalLit(tip * Math.cos(0.3), tip * Math.sin(0.3), 0.3))
        assertTrue(!PetalLayout.petalLit(0.0, 10.0, 0.0))
        // longer along its outward direction (half-length 5.2, the notch from 4.1) than across it (4.3)
        assertTrue(PetalLayout.petalLit(4.0, 0.0, 0.0) && !PetalLayout.petalLit(0.0, 4.5, 0.0))
    }
}
