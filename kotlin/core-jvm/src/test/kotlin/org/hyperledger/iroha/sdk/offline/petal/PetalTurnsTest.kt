package org.hyperledger.iroha.sdk.offline.petal

import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Ports of the reference `turns` unit tests plus the turned cells, renders and draw lists. */
class PetalTurnsTest {
    private fun atoms(seed: Int): ByteArray =
        ByteArray(PetalTurns.TURN_ATOMS * PetalLanes.ATOM_LEN) { ((it * 29) xor seed).toByte() }

    @Test
    fun laneSizesAreConsistent() {
        assertEquals(90 to 68, PetalTurns.TURN_WORD to PetalTurns.TURN_DATA)
        assertEquals(1L shl 31, PetalTestSupport.u32(PetalTurns.turnFirstId(0, 0)))
        assertEquals((1L shl 31) + 16 + 12, PetalTestSupport.u32(PetalTurns.turnFirstId(1, 3)))
        assertEquals((1L shl 31) + 16L * 65_535 + 15, PetalTestSupport.u32(PetalTurns.turnFirstId(65_535, 3) + 3))
        // the base schedule stays far below the turn ids however long a stream runs
        assertTrue(PetalStream.firstAtomId(65_535).toLong() + 21 < PetalTestSupport.u32(PetalTurns.TURN_ID_BASE))
        assertFailsWith<IllegalArgumentException> { PetalTurns.turnFirstId(65_536, 0) }
        assertFailsWith<IllegalArgumentException> { PetalTurns.turnFirstId(0, 4) }
    }

    @Test
    fun grayLabelsDifferInOneBitBetweenNeighbouringTurns() {
        for (position in 0 until PetalTurns.TURN_POSITIONS) {
            assertEquals(position, PetalTurns.turnPosition(PetalTurns.turnLabel(position)))
            val next = (position + 1) % PetalTurns.TURN_POSITIONS
            assertEquals(1, Integer.bitCount(PetalTurns.turnLabel(position) xor PetalTurns.turnLabel(next)))
        }
        assertTrue(Math.abs(PetalTurns.turnAngle(2) - Math.PI / 2) < 1e-12)
    }

    @Test
    fun carrierOrdersShuffleEachSectorsCodewordCells() {
        for (sector in 0 until PetalLayout.SECTORS) {
            val order = PetalTurns.carrierOrder(sector)
            val cells = PetalLayout.sectorCells(sector).copyOf(PetalLanes.FIELD_WORD * 8)
            assertTrue(!order.contentEquals(cells), "sector $sector is shuffled")
            assertContentEquals(cells.sortedArray(), order.sortedArray(), "sector $sector holds its codeword cells")
        }
        // the four shuffles differ
        fun positions(sector: Int): List<Int> {
            val cells = PetalLayout.sectorCells(sector)
            return PetalTurns.carrierOrder(sector).map { cell -> cells.indexOf(cell) }
        }
        assertTrue(positions(0) != positions(1))
        assertFailsWith<IllegalArgumentException> { PetalTurns.carrierOrder(4) }
    }

    @Test
    fun aSectorWithTooFewLitPetalsCarriesNothing() {
        val petals = BooleanArray(PetalLayout.FIELD_CELLS)
        val order = PetalTurns.carrierOrder(2)
        for (cell in order.copyOf(PetalTurns.TURN_CARRIERS - 1)) petals[cell] = true
        assertNull(PetalTurns.carriers(petals, 2))
        petals[order[PetalTurns.TURN_CARRIERS + 7]] = true
        val chosen = assertNotNull(PetalTurns.carriers(petals, 2), "enough lit petals")
        assertEquals(PetalTurns.TURN_CARRIERS, chosen.size)
        assertEquals(order[PetalTurns.TURN_CARRIERS + 7], chosen[PetalTurns.TURN_CARRIERS - 1])
    }

    @Test
    fun turnLanesRoundtripAndBindTheirHeader() {
        val header = PetalLaneHeader(7, 300)
        val data = PetalTurns.turnData(header, 1, atoms(3))
        val word = PetalTurns.encodeTurnLane(1, data)
        assertEquals(PetalTurns.TURN_WORD, word.size)
        val decoded = PetalTurns.decodeTurnLaneCounted(1, word)
        assertContentEquals(data, decoded.data)
        assertEquals(0, decoded.corrected)
        val packet = assertNotNull(PetalTurns.turnPacket(header, 1, decoded.data), "check matches")
        assertEquals(PetalTurns.turnFirstId(300, 1), packet.firstId)
        assertEquals(PetalTurns.TURN_ATOMS, packet.atomCount)
        assertContentEquals(atoms(3).copyOfRange(32, 48), packet.atom(2))
        // another frame, tag or sector does not match the check
        assertNull(PetalTurns.turnPacket(PetalLaneHeader(7, 301), 1, decoded.data))
        assertNull(PetalTurns.turnPacket(PetalLaneHeader(8, 300), 1, decoded.data))
        assertNull(PetalTurns.turnPacket(header, 2, decoded.data))
        assertNull(PetalTurns.turnPacket(header, 1, decoded.data.copyOf(67)))
        // a codeword whitened for another sector does not decode
        assertEquals(
            PetalReedSolomonException.Reason.UNCORRECTABLE,
            assertFailsWith<PetalReedSolomonException> { PetalTurns.decodeTurnLaneCounted(0, word) }.reason,
        )
        // damaged bytes are corrected, flagged ones as erasures
        val damaged = word.copyOf()
        for (position in intArrayOf(0, 9, 44, 89)) damaged[position] = (damaged[position].toInt() xor 0xC3).toByte()
        val repaired = PetalTurns.decodeTurnLaneCounted(1, damaged, intArrayOf(9, 44))
        assertContentEquals(data, repaired.data)
        assertEquals(4 to 2, repaired.corrected to repaired.erasures)
        assertEquals(
            PetalReedSolomonException.Reason.INVALID_SHAPE,
            assertFailsWith<PetalReedSolomonException> { PetalTurns.decodeTurnLaneCounted(1, word.copyOf(89)) }.reason,
        )
        assertFailsWith<IllegalArgumentException> { PetalTurns.turnData(header, 1, ByteArray(63)) }
        assertFailsWith<IllegalArgumentException> { PetalTurns.encodeTurnLane(1, ByteArray(67)) }
    }

    @Test
    fun carriersShowTheCodewordTwoBitsAtATime() {
        val fields = List(PetalLayout.SECTORS) { sector ->
            PetalLanes.encodeLane(PetalLane.field(sector), ByteArray(PetalLanes.FIELD_DATA) { ((it * 13) xor sector).toByte() })
        }
        val plain = PetalFrameCells.fromWords(ByteArray(PetalLanes.D_WORD), fields)
        assertTrue(plain.turns().all { it.toInt() == 0 })
        val words = List(PetalLayout.SECTORS) { sector ->
            PetalTurns.encodeTurnLane(sector, PetalTurns.turnData(PetalLaneHeader(1, 2), sector, atoms(sector)))
        }
        val cells = plain.withTurns(words)
        assertTrue(plain.turns().all { it.toInt() == 0 }, "withTurns leaves the original unturned")
        for ((sector, word) in words.withIndex()) {
            assertContentEquals(word, cells.turnWord(sector))
            val chosen = assertNotNull(PetalTurns.carriers(cells.petals(), sector))
            for ((j, cell) in chosen.withIndex()) assertEquals(PetalTurns.carrierPosition(word, j), cells.turn(cell))
        }
        // dark petals and lit petals beyond the carriers stay unturned
        var carried = 0
        for (sector in 0 until PetalLayout.SECTORS) {
            carried += assertNotNull(PetalTurns.carriers(cells.petals(), sector)).count { cells.turn(it) != 0 }
        }
        val turned = (0 until PetalLayout.FIELD_CELLS).count { cells.turn(it) != 0 }
        assertEquals(carried, turned)
        assertTrue(turned > PetalLayout.SECTORS * PetalTurns.TURN_CARRIERS * 6 / 10)
        // turns are part of the cells' identity
        assertTrue(cells != plain)
        assertEquals(cells, PetalFrameCells.of(cells.dots(), cells.petals(), cells.turns()))
        assertEquals(plain, PetalFrameCells.of(cells.dots(), cells.petals()))
        assertFailsWith<IllegalArgumentException> { PetalFrameCells.of(cells.dots(), cells.petals(), ByteArray(PetalLayout.FIELD_CELLS) { 4 }) }
        assertFailsWith<IllegalArgumentException> { plain.withTurns(words.take(3)) }
        assertFailsWith<IllegalArgumentException> { plain.withTurns(words.map { it.copyOf(89) }) }
    }

    @Test
    fun aTurnedPetalStaysOutOfItsNeighboursSamplingDiscs() {
        // the nearest petal centres (radially adjacent cells) are 11 units apart; a petal reaches
        // its half-length from its centre whichever way it is turned
        var nearest = Double.MAX_VALUE
        for (entry in PetalTurnTables.NEAR_CELL.indices) {
            nearest = minOf(nearest, Math.hypot(PetalTurnTables.NEAR_ALONG[entry], PetalTurnTables.NEAR_ACROSS[entry]))
        }
        assertTrue(nearest > (PetalLayout.PETAL_HALF_LENGTH.toFloat() + PetalLayout.PETAL_RADIUS.toFloat()).toDouble(), "$nearest")
        assertEquals(11.0, nearest, 1e-9)
        // the reference's neighbour table has 22,976 entries
        assertEquals(22_976, PetalTurnTables.NEAR_CELL.size)
    }

    private val stream = PetalStreamEncoder(PetalTestSupport.payload(6_000, 5), 2)

    private fun rendered(cells: PetalFrameCells): PetalRgb = PetalRenderer.render(cells, PetalRenderOptions(768, 2))

    @Test
    fun aCleanRenderReadsEveryTurnLaneAndSkipsWhatItIsTold() {
        val truth = stream.laneData(6)
        val image = rendered(stream.cells(6)).toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(image).frame, "decodes")
        assertEquals("D0123", decoded.lanes)
        val turns = PetalTurns.readTurns(image, decoded)
        for (sector in 0 until PetalLayout.SECTORS) {
            val result = assertNotNull(turns[sector], "turn lane $sector")
            assertContentEquals(truth.turn(sector), result.data)
            assertEquals(0, result.corrected)
        }
        val skipped = PetalTurns.readTurns(image, decoded, booleanArrayOf(true, false, true, true))
        assertEquals(listOf(false, true, false, false), skipped.map { it != null })
        // a sector whose field lane did not decode has no turn lane
        val partial = PetalDecodedFrame(
            decoded.pose,
            decoded.rotation,
            decoded.mirrored,
            decoded.d,
            listOf(decoded.field(0), decoded.field(1), null, decoded.field(3)),
            decoded.inferredCorner,
        )
        val turnsOfPartial = PetalTurns.readTurns(image, partial)
        assertTrue(turnsOfPartial[2] == null && turnsOfPartial[3] != null)
        // nothing to read without decoded field lanes, and nothing when every sector is skipped
        val bare = PetalDecodedFrame(decoded.pose, decoded.rotation, decoded.mirrored, decoded.d, listOf(null, null, null, null), null)
        assertTrue(PetalTurns.readTurns(image, bare).all { it == null })
        assertNull(PetalTurns.observedTurns(image, bare))
        assertTrue(PetalTurns.readTurns(image, decoded, BooleanArray(4) { true }).all { it == null })
        assertFailsWith<IllegalArgumentException> { PetalTurns.readTurns(image, decoded, BooleanArray(3)) }
        // what the reader decided agrees with what was drawn
        val observed = assertNotNull(PetalTurns.observedTurns(image, decoded))
        assertEquals(PetalTurns.TURN_BLURS[0], observed.blur)
        val cells = stream.cells(6)
        assertTrue(observed.carriers.all { cells.turn(it.cell) == it.position && it.lead > 0.0 })
    }

    @Test
    fun aModernCameraReadsTheTurnLanes() {
        val truth = stream.laneData(9)
        val source = rendered(stream.cells(9))
        for ((seed, rotation) in listOf(1L to 17.0, 2L to 205.0)) {
            val config = PetalCameraSimulator.fitToFrame(
                PetalCaptureConfig.modern().copy(rotationDeg = rotation, tiltXDeg = 8.0, seed = seed),
                4.0,
            )
            val image = PetalCameraSimulator.capture(source, config)
            val decoded = assertNotNull(PetalDecoder.decode(image).frame, "seed $seed")
            val turns = PetalTurns.readTurns(image, decoded)
            for (sector in 0 until PetalLayout.SECTORS) {
                if (decoded.field(sector) != null) {
                    assertContentEquals(truth.turn(sector), turns[sector]?.data, "seed $seed sector $sector")
                }
            }
        }
    }

    @Test
    fun unturnedPetalsNeverPassForATurnLane() {
        val cells = stream.cells(3)
        val image = rendered(PetalFrameCells.of(cells.dots(), cells.petals())).toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(image).frame, "the field lanes read without turns")
        assertEquals("D0123", decoded.lanes)
        assertTrue(PetalTurns.readTurns(image, decoded).all { it == null })
    }

    @Test
    fun templatesHoldAUnitPetalAndKernelsTellTheTurnsApart() {
        assertTrue(PetalTurns.turnTemplate(0, 0.0, 0.0) > 0.99)
        assertTrue(PetalTurns.turnTemplate(0, 0.0, 6.0) < 0.05 && PetalTurns.turnTemplate(0, 8.5, 0.0) < 0.01)
        assertTrue(PetalTurns.turnTemplate(0, 4.5, 0.0) > PetalTurns.turnTemplate(0, 0.0, 3.4))
        // zero outside the table
        assertEquals(0.0, PetalTurns.turnTemplate(5, 9.0, 0.0))
        assertEquals(0.0, PetalTurns.turnTemplate(5, 0.0, -9.01))
        assertFailsWith<IllegalArgumentException> { PetalTurns.turnTemplate(6, 0.0, 0.0) }
        val reader = PetalTurnReader()
        for (blur in PetalTurns.TURN_BLURS.indices) {
            for (position in 0 until PetalTurns.TURN_POSITIONS) {
                // a window showing exactly the petal at turn k picks turn k
                val kernel = (blur * PetalTurns.TURN_POSITIONS + position) * PetalTurnTables.POINTS
                assertEquals(position, reader.decide(PetalTurnTables.KERNELS, kernel, blur), "blur $blur position $position")
                assertTrue(reader.lead > 0.0)
            }
        }
    }

    @Test
    fun turnedRendersAndDrawListsMatchTheReference() {
        // CRC-32C of the Rust reference `render` output (the whole card) of the turn tests' frames
        val expected = listOf(
            stream.cells(6) to 0x913B_4A27.toInt(),
            stream.cells(9) to 0x4AE3_1996,
            PetalFrameCells.of(stream.cells(3).dots(), stream.cells(3).petals()) to 0xB234_98B5.toInt(),
        )
        for ((cells, crc) in expected) assertEquals(crc, PetalCrc32c.compute(rendered(cells).data()))
        // a draw list turns its petals as the renderer does, and shares them between frames
        val cells = stream.cells(6)
        val list = PetalDrawList.of(cells)
        assertEquals(cells.petals().count { it }, list.petals.size)
        var turned = 0
        for (petal in list.petals) {
            assertEquals(cells.turn(petal.cell), petal.turn)
            assertEquals(PetalLayout.cellAngle(petal.cell) + PetalTurns.turnAngle(petal.turn), petal.angle)
            if (petal.turn != 0) turned += 1
        }
        assertTrue(turned > PetalLayout.SECTORS * PetalTurns.TURN_CARRIERS / 2)
        val again = PetalDrawList.of(cells)
        assertTrue(list.petals.indices.all { list.petals[it] === again.petals[it] }, "petals are shared between lists")
    }
}
