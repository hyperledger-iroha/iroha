package org.hyperledger.iroha.sdk.offline.petal

import java.nio.ByteBuffer
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

/** Ports of the reference `image`, `geometry`, `locate`, `decode` and `render` tests plus robustness tests. */
class PetalImageDecodeTest {
    @Test
    fun bilinearSamplingInterpolatesBetweenPixelCentres() {
        val image = assertNotNull(PetalLuma.fromRaw(2, 1, byteArrayOf(0, 100)))
        assertEquals(0.0, image.sample(0.5, 0.5), 1e-9)
        assertEquals(100.0, image.sample(1.5, 0.5), 1e-9)
        assertEquals(50.0, image.sample(1.0, 0.5), 1e-9)
        assertEquals(0.0, image.sample(-5.0, 9.0), 1e-9)
        assertTrue(image.sample(Double.NaN, 0.5).isNaN(), "NaN coordinates propagate like the reference")
        assertFailsWith<IllegalStateException> { PetalLuma(0, 0).sample(0.0, 0.0) }
    }

    @Test
    fun stridedPlanesDropThePadding() {
        val plane = byteArrayOf(1, 2, 9, 9, 3, 4, 9, 9)
        assertContentEquals(byteArrayOf(1, 2, 3, 4), assertNotNull(PetalLuma.fromStrided(2, 2, 4, plane)).data())
        assertNull(PetalLuma.fromStrided(2, 2, 1, plane))
        assertNull(PetalLuma.fromStrided(2, 3, 4, plane))
        assertNull(PetalLuma.fromStrided(2, 0, 4, plane))
        assertNull(PetalLuma.fromRaw(100, 100, ByteArray(5)))
    }

    @Test
    fun planesWithPixelStridesAndRotationsAreCopied() {
        // a 3x2 plane with pixel stride 2 and row stride 7, after a 1-byte buffer offset
        val raw = byteArrayOf(99, 1, 0, 2, 0, 3, 0, 0, 4, 0, 5, 0, 6, 0)
        val buffer = ByteBuffer.wrap(raw)
        buffer.position(1)
        val upright = PetalLuma.fromPlane(buffer, 3, 2, 7, 2, 0)
        assertEquals(1, buffer.position(), "the caller's buffer position is untouched")
        assertContentEquals(byteArrayOf(1, 2, 3, 4, 5, 6), upright.data())
        val clockwise = PetalLuma.fromPlane(buffer, 3, 2, 7, 2, 90)
        assertEquals(2, clockwise.width)
        assertEquals(3, clockwise.height)
        assertContentEquals(byteArrayOf(4, 1, 5, 2, 6, 3), clockwise.data())
        assertContentEquals(byteArrayOf(6, 5, 4, 3, 2, 1), PetalLuma.fromPlane(buffer, 3, 2, 7, 2, 180).data())
        assertContentEquals(byteArrayOf(3, 6, 2, 5, 1, 4), PetalLuma.fromPlane(buffer, 3, 2, 7, 2, 270).data())
        val packed = PetalLuma.fromPlane(ByteBuffer.wrap(byteArrayOf(1, 2, 3, 4, 5, 6)), 3, 2, 3)
        assertContentEquals(byteArrayOf(1, 2, 3, 4, 5, 6), packed.data())
        assertFailsWith<IllegalArgumentException> { PetalLuma.fromPlane(buffer, 3, 2, 7, 2, 45) }
        assertFailsWith<IllegalArgumentException> { PetalLuma.fromPlane(buffer, 3, 3, 7, 2, 0) }
        assertFailsWith<IllegalArgumentException> { PetalLuma.fromPlane(buffer, 3, 2, 4, 2, 0) }
    }

    @Test
    fun rgbLumaUsesRec601Weights() {
        val rgb = assertNotNull(PetalRgb.fromRaw(3, 1, byteArrayOf(-1, 0, 0, 0, -1, 0, 0, 0, -1)))
        assertContentEquals(byteArrayOf(76, 150.toByte(), 29), rgb.toLuma().data())
        assertNull(PetalRgb.fromRaw(3, 1, ByteArray(8)))
    }

    @Test
    fun fourPointsAreMappedExactly() {
        val src = doubleArrayOf(0.0, 0.0, 1024.0, 0.0, 1024.0, 1024.0, 0.0, 1024.0)
        val dst = doubleArrayOf(103.5, 40.25, 590.0, 70.0, 560.0, 420.0, 80.0, 380.0)
        val h = assertNotNull(PetalHomography.fromPoints(src, dst))
        for (point in 0 until 4) {
            val mapped = h.apply(src[2 * point], src[2 * point + 1])
            assertEquals(dst[2 * point], mapped[0], 1e-7)
            assertEquals(dst[2 * point + 1], mapped[1], 1e-7)
        }
    }

    @Test
    fun inverseRoundtripsAndLeastSquaresAveragesNoise() {
        val truth = PetalHomography(doubleArrayOf(0.4, -0.1, 130.0, 0.12, 0.38, 60.0, 1e-4, -2e-5, 1.0))
        val src = DoubleArray(60)
        val dst = DoubleArray(60)
        for (i in 0 until 30) {
            src[2 * i] = 50.0 + 31.0 * (i % 6)
            src[2 * i + 1] = 90.0 + 47.0 * (i / 6)
            val mapped = truth.apply(src[2 * i], src[2 * i + 1])
            val jitter = if (i % 2 == 0) 0.05 else -0.05
            dst[2 * i] = mapped[0] + jitter
            dst[2 * i + 1] = mapped[1] - jitter
        }
        val fit = assertNotNull(PetalHomography.fromPoints(src, dst))
        for (i in 0 until 30) {
            val expected = truth.apply(src[2 * i], src[2 * i + 1])
            val actual = fit.apply(src[2 * i], src[2 * i + 1])
            assertEquals(expected[0], actual[0], 0.1)
            assertEquals(expected[1], actual[1], 0.1)
        }
        val inverse = assertNotNull(truth.inverse())
        val forward = truth.apply(300.0, 200.0)
        val back = inverse.apply(forward[0], forward[1])
        assertEquals(300.0, back[0], 1e-6)
        assertEquals(200.0, back[1], 1e-6)
        assertEquals(truth, truth.compose(PetalHomography.IDENTITY))
    }

    @Test
    fun degenerateHomographyInputsAreRejected() {
        val p = DoubleArray(8) { 1.0 }
        assertNull(PetalHomography.fromPoints(p, p))
        assertNull(PetalHomography.fromPoints(p.copyOf(6), p.copyOf(6)))
        assertNull(PetalHomography.fromPoints(p, p.copyOf(10)))
        assertNull(PetalHomography(DoubleArray(9)).inverse())
        assertFailsWith<IllegalArgumentException> { PetalHomography(DoubleArray(8)) }
    }

    @Test
    fun findsTheFourCornerBlossomsInACleanRender() {
        val encoder = PetalStreamEncoder(ByteArray(200) { 9 }, 1)
        val luma = PetalTestSupport.renderLuma(encoder, 1, 512, 2)
        val quad = assertNotNull(PetalLocator.locate(luma), "four finders")
        val expected = listOf(36.0 to 36.0, 476.0 to 36.0, 476.0 to 476.0, 36.0 to 476.0)
        quad.zip(expected).forEach { (finder, position) ->
            assertTrue(Math.abs(finder.x - position.first) < 1.5 && Math.abs(finder.y - position.second) < 1.5, "$finder")
            assertTrue(Math.abs(finder.size - 60.0) < 6.0, "size ${finder.size}")
        }
        // the public pipeline stages compose to the same answer
        val components = PetalLocator.labelComponents(PetalLocator.adaptiveBinarize(luma, 0.12), 512, 512)
        val selected = assertNotNull(PetalLocator.selectQuad(PetalLocator.blossoms(components)))
        assertEquals(quad, selected.map { PetalLocator.refineCenter(luma, it) })
    }

    @Test
    fun componentsAreLabelledWithCorrectGeometry() {
        val mask = BooleanArray(64)
        for (y in 1 until 4) for (x in 2 until 6) mask[y * 8 + x] = true
        mask[6 * 8 + 6] = true
        val components = PetalLocator.labelComponents(mask, 8, 8)
        assertEquals(2, components.size)
        val big = components.single { it.area == 12 }
        assertEquals(listOf(2, 5, 1, 3), listOf(big.minX, big.maxX, big.minY, big.maxY))
        assertEquals(4.0, big.centroidX, 1e-9)
        assertEquals(2.5, big.centroidY, 1e-9)
        assertEquals(4.0, big.width)
        assertEquals(3.0, big.height)
        // a U shape merges two provisional labels into the smaller one
        val u = BooleanArray(9)
        for (index in intArrayOf(0, 2, 3, 5, 6, 7, 8)) u[index] = true
        assertEquals(listOf(7), PetalLocator.labelComponents(u, 3, 3).map { it.area })
        assertFailsWith<IllegalArgumentException> { PetalLocator.labelComponents(BooleanArray(5), 2, 2) }
    }

    @Test
    fun orderingIsClockwiseFromTheTopLeft() {
        val f = { x: Double, y: Double -> PetalFinder(x, y, 10.0) }
        val quad = assertNotNull(PetalLocator.selectQuad(listOf(f(90.0, 90.0), f(10.0, 12.0), f(88.0, 8.0), f(12.0, 92.0))))
        assertEquals(listOf(10.0 to 12.0, 88.0 to 8.0, 90.0 to 90.0, 12.0 to 92.0), quad.map { it.x to it.y })
        assertNull(PetalLocator.selectQuad(quad.take(3)))
    }

    @Test
    fun theLargestCandidatesWinWhenClutterPrecedesThem() {
        // twelve small decoys discovered before the four real finders
        val candidates = (0 until 12).map { PetalFinder(10.0 + 7.0 * it, 5.0, 18.0) } + listOf(
            PetalFinder(100.0, 100.0, 60.0),
            PetalFinder(700.0, 110.0, 62.0),
            PetalFinder(690.0, 520.0, 58.0),
            PetalFinder(95.0, 510.0, 61.0),
        )
        val quad = assertNotNull(PetalLocator.selectQuad(candidates), "real finders found")
        assertEquals(listOf(95L, 100L, 690L, 700L), quad.map { it.x.toLong() }.sorted())
        // decoys large enough to pass the size-class filter must not crowd the real finders out either
        val bigDecoys = (0 until 12).map { PetalFinder(10.0 + 9.0 * it, 5.0, 40.0) } + candidates.takeLast(4)
        val quadAmongBigDecoys = assertNotNull(PetalLocator.selectQuad(bigDecoys), "real finders ranked first")
        assertEquals(listOf(95L, 100L, 690L, 700L), quadAmongBigDecoys.map { it.x.toLong() }.sorted())
    }

    @Test
    fun aBlankImageHasNoFinders() {
        assertNull(PetalLocator.locate(PetalLuma(200, 200)))
        assertNull(PetalLocator.locate(PetalLuma(0, 0)))
    }

    private val streamPayload = ByteArray(300) { i -> ((i.toLong() * 2_654_435_761L) ushr 11).toByte() }

    private fun setup(frame: Int): Pair<PetalStreamEncoder, PetalRgb> {
        val encoder = PetalStreamEncoder(streamPayload, 2)
        return encoder to PetalRenderer.render(encoder.cells(frame), PetalRenderOptions(768, 2))
    }

    @Test
    fun cleanRenderDecodesEveryLane() {
        val (encoder, rgb) = setup(5)
        val luma = rgb.toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame)
        val data = encoder.laneData(5)
        assertContentEquals(data.p(), decoded.p?.data)
        assertContentEquals(data.k(), decoded.k?.data)
        assertContentEquals(data.d(), decoded.d?.data)
        assertEquals(0, decoded.rotation)
        assertEquals(false, decoded.mirrored)
        assertEquals(3, decoded.lanesOk)
        assertEquals("PKD", decoded.lanes)
        // diagnostics agree with what was drawn (Reed–Solomon may have corrected a few glyphs)
        val truth = encoder.cells(5)
        val observed = assertNotNull(PetalDecoder.observedCells(luma, decoded))
        assertContentEquals(truth.light(), observed.light())
        assertContentEquals(truth.dots(), observed.dots())
        assertTrue((0 until PetalLayout.TILE_COUNT).count { truth.glyph(it) != observed.glyph(it) } <= 4)
        val matchError = assertNotNull(PetalDecoder.tileMatchError(luma, decoded))
        assertTrue(matchError >= 0.0 && matchError < 1.0, "tile match error $matchError")
        val again = assertNotNull(PetalDecoder.decodeAt(luma, decoded.homography))
        assertEquals("PKD", again.lanes)
        // frame 5 carries atoms on every lane
        assertNull(decoded.beacon())
        assertEquals(3, decoded.atomPackets().size)
        assertEquals(7, decoded.atomPackets().sumOf { it.atomCount })
    }

    @Test
    fun rotatedAndMirroredRendersDecodeWithTheRightOrientation() {
        val (encoder, rgb) = setup(7)
        val data = encoder.laneData(7)
        val n = rgb.width
        val source = rgb.toLuma().data()
        fun transform(map: (Int, Int) -> Pair<Int, Int>): PetalLuma {
            val out = ByteArray(n * n)
            for (y in 0 until n) {
                for (x in 0 until n) {
                    val (sx, sy) = map(x, y)
                    out[y * n + x] = source[sy * n + sx]
                }
            }
            return assertNotNull(PetalLuma.fromRaw(n, n, out))
        }
        // mirrored hypotheses enumerate corners in the opposite direction, so the
        // unrotated mirror reports quarter-turn index 1
        val cases = listOf(
            OrientationCase("rot90", transform { x, y -> y to (n - 1 - x) }, 1, false),
            OrientationCase("rot180", transform { x, y -> (n - 1 - x) to (n - 1 - y) }, 2, false),
            OrientationCase("rot270", transform { x, y -> (n - 1 - y) to x }, 3, false),
            OrientationCase("mirror", transform { x, y -> (n - 1 - x) to y }, 1, true),
        )
        for ((name, image, rotation, mirrored) in cases) {
            val result = PetalDecoder.decode(image)
            val decoded = assertNotNull(result.frame, "$name: ${result.error}")
            assertEquals(mirrored, decoded.mirrored, name)
            assertEquals(rotation, decoded.rotation, "$name rotation")
            assertContentEquals(data.d(), decoded.d?.data, "$name lane D")
            assertContentEquals(data.p(), decoded.p?.data, "$name lane P")
        }
        // without mirrored hypotheses a mirrored preview is unreadable
        val strict = PetalDecodeOptions(tryMirrored = false)
        assertEquals(PetalDecodeError.NO_ORIENTATION, PetalDecoder.decode(cases.last().image, strict).error)
    }

    private data class OrientationCase(val name: String, val image: PetalLuma, val rotation: Int, val mirrored: Boolean)

    @Test
    fun correctedCountsRewrittenBytesNotJustErasures() {
        val data = ByteArray(PetalLanes.P_DATA) { it.toByte() }
        val word = PetalLanes.encodeLane(PetalLane.P, data)
        for (position in intArrayOf(2, 11, 30)) word[position] = (word[position].toInt() xor 0x5A).toByte()
        val confidence = DoubleArray(word.size) { 1.0 }
        val result = assertNotNull(PetalDecoder.decodeWithErasures(PetalLane.P, word, confidence), "three errors fit")
        assertContentEquals(data, result.data)
        assertEquals(0, result.erasures)
        assertEquals(3, result.corrected)
        // with the damaged bytes flagged as least confident, they become erasures
        val flagged = DoubleArray(word.size) { 1.0 }
        for (position in intArrayOf(2, 11, 30)) flagged[position] = 0.0
        val erased = assertNotNull(PetalDecoder.decodeWithErasures(PetalLane.P, word, flagged), "decodes")
        assertContentEquals(data, erased.data)
        assertTrue(erased.corrected >= 3)
    }

    /** The exact canvas-to-pixel homography of the 768-pixel test renders. */
    private fun renderHomography(): PetalHomography {
        val canonical = DoubleArray(8)
        for (finder in 0 until 4) {
            canonical[2 * finder] = PetalLayout.finderCenterX(finder)
            canonical[2 * finder + 1] = PetalLayout.finderCenterY(finder)
        }
        val scale = 768.0 / 1024.0
        return assertNotNull(PetalHomography.fromPoints(canonical, DoubleArray(8) { canonical[it] * scale }), "homography")
    }

    /** A clean 768-pixel render with the exact canvas-to-pixel homography; its raw patches are in [workspace]. */
    private class CleanPatches(
        val encoder: PetalStreamEncoder,
        val luma: PetalLuma,
        val h: PetalHomography,
        val workspace: PetalWorkspace,
    )

    private fun cleanPatches(frame: Int): CleanPatches {
        val (encoder, rgb) = setup(frame)
        val luma = rgb.toLuma()
        val h = renderHomography()
        val workspace = PetalWorkspace()
        PetalDecoder.samplePatches(luma, h.m, workspace)
        return CleanPatches(encoder, luma, h, workspace)
    }

    private fun laneOf(lane: PetalLane, words: PetalDecoder.TileWords): PetalLaneResult? = when (lane) {
        PetalLane.P -> PetalDecoder.decodeWithErasures(lane, words.p, words.pConfidence)
        PetalLane.K -> PetalDecoder.decodeWithErasures(lane, words.k, words.kConfidence)
        PetalLane.D -> throw IllegalArgumentException("lane D is not read from tiles")
    }

    @Test
    fun levelAndNormalisedReadsAgreeOnACleanRender() {
        val clean = cleanPatches(5)
        val data = clean.encoder.laneData(5)
        val options = PetalDecodeOptions.DEFAULT
        val reference = assertNotNull(PetalDecoder.referenceLevels(clean.luma, clean.h.m), "reference levels")
        val reads = listOf(
            "level" to PetalDecoder.readTiles(reference, options, clean.workspace),
            "normalised" to PetalDecoder.readTilesNormalised(options, clean.workspace),
        )
        for ((name, tiles) in reads) {
            val words = PetalDecoder.tileWords(tiles)
            val p = assertNotNull(laneOf(PetalLane.P, words), "$name: lane P")
            val k = assertNotNull(laneOf(PetalLane.K, words), "$name: lane K")
            assertContentEquals(data.p(), p.data, name)
            assertEquals(0, p.corrected, name)
            assertContentEquals(data.k(), k.data, name)
            assertEquals(0, k.corrected, name)
        }
    }

    @Test
    fun normalisedReadCancelsGainAndOffsetPerTile() {
        val clean = cleanPatches(5)
        val data = clean.encoder.laneData(5)
        val options = PetalDecodeOptions.DEFAULT
        // every tile gets its own gain and offset, as under glare, shadows and saturation
        val patches = clean.workspace.patches
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val gain = 0.35 + 0.65 * ((tile * 37 % 101).toDouble() / 100.0)
            val offset = 5.0 + (tile * 53 % 61).toDouble()
            for (cell in tile * PetalWorkspace.CELLS until (tile + 1) * PetalWorkspace.CELLS) {
                patches[cell] = gain * patches[cell] + offset
            }
        }
        val reference = assertNotNull(PetalDecoder.referenceLevels(clean.luma, clean.h.m), "reference levels")
        val levelWords = PetalDecoder.tileWords(PetalDecoder.readTiles(reference, options, clean.workspace))
        assertNull(
            laneOf(PetalLane.P, levelWords),
            "the level read must not survive this distortion, or the test proves nothing",
        )
        val words = PetalDecoder.tileWords(PetalDecoder.readTilesNormalised(options, clean.workspace))
        assertContentEquals(data.p(), assertNotNull(laneOf(PetalLane.P, words), "lane P").data)
        assertContentEquals(data.k(), assertNotNull(laneOf(PetalLane.K, words), "lane K").data)
    }

    @Test
    fun normalisedReadErasesTilesThatLostTheirContrast() {
        val clean = cleanPatches(5)
        val cells = PetalWorkspace.CELLS
        clean.workspace.patches.fill(100.0, 5 * cells, 6 * cells)
        clean.workspace.patches.fill(30.0, 9 * cells, 10 * cells)
        val reads = PetalDecoder.readTilesNormalised(PetalDecodeOptions.DEFAULT, clean.workspace)
        for (tile in intArrayOf(5, 9)) {
            assertTrue(Math.abs(reads.polarityMargin[tile]) < 1e-12, "tile $tile")
            assertTrue(Math.abs(reads.glyphMargin[tile]) < 1e-12, "tile $tile")
        }
        assertTrue(reads.polarityMargin[6] > 0.0 && reads.glyphMargin[6] > 0.0)
    }

    @Test
    fun normalisedReadJudgesContrastAgainstTheMedianTile() {
        val clean = cleanPatches(5)
        val cells = PetalWorkspace.CELLS
        val patches = clean.workspace.patches
        // 100 tiles keep 15 % of their contrast (a deep shadow patch), the other 156 stay healthy
        for (cell in 0 until 100 * cells) patches[cell] = 0.15 * patches[cell] + 40.0
        val reads = PetalDecoder.readTilesNormalised(PetalDecodeOptions.DEFAULT, clean.workspace)
        // the median tile is a healthy one, so every weak tile is below a quarter of it and erased ...
        for (tile in 0 until 100) {
            assertTrue(Math.abs(reads.polarityMargin[tile]) < 1e-12, "weak tile $tile")
            assertTrue(Math.abs(reads.glyphMargin[tile]) < 1e-12, "weak tile $tile")
        }
        // ... while the healthy tiles keep their margins
        for (tile in 100 until PetalLayout.TILE_COUNT) assertTrue(reads.polarityMargin[tile] > 0.0, "healthy tile $tile")
    }

    @Test
    fun patchLevelsIgnoreTheExtremeCells() {
        val values = DoubleArray(PetalWorkspace.CELLS) { 10.0 }
        for (i in 0 until PetalWorkspace.CELLS / 2) values[i] = 200.0 + (i % 3)
        values[0] = 255.0 // one hot cell
        values[PetalWorkspace.CELLS - 1] = 0.0 // one dead cell
        val (low, high) = PetalDecoder.patchLevels(values)
        assertEquals(10.0, low, 1e-12)
        assertTrue(high in 200.0..202.0, "high level $high")
        val scaled = PetalDecoder.rescale(values, 1.0)
        assertTrue(scaled.all { it in -0.25..1.25 })
        // a flat patch stays flat instead of dividing by nothing
        assertTrue(PetalDecoder.rescale(DoubleArray(PetalWorkspace.CELLS) { 7.0 }, 1.0).all { Math.abs(it) < 1e-12 })
    }

    @Test
    fun shadowedPartOfARenderDecodesThroughTheNormalisedRead() {
        val (encoder, rgb) = setup(5)
        val data = encoder.laneData(5)
        val clear = rgb.toLuma()
        val width = clear.width
        val pixels = clear.data()
        for (y in 0 until clear.height) {
            for (x in width * 7 / 20 until width * 3 / 5) {
                val dimmed = PetalNumerics.roundHalfAway((pixels[y * width + x].toInt() and 0xFF) * 0.3)
                pixels[y * width + x] = dimmed.toInt().toByte()
            }
        }
        val luma = assertNotNull(PetalLuma.fromRaw(width, clear.height, pixels))
        // the finder levels cannot describe a step in the light: the level read loses lane K
        val h = renderHomography()
        val options = PetalDecodeOptions.DEFAULT
        val reference = assertNotNull(PetalDecoder.referenceLevels(luma, h.m), "reference levels")
        val workspace = PetalWorkspace()
        PetalDecoder.samplePatches(luma, h.m, workspace)
        val levelWords = PetalDecoder.tileWords(PetalDecoder.readTiles(reference, options, workspace))
        assertNull(laneOf(PetalLane.K, levelWords), "the level read must lose lane K")
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        assertContentEquals(data.p(), decoded.p?.data)
        assertContentEquals(data.k(), decoded.k?.data)
    }

    @Test
    fun aFrameWhoseOnlyReadableLaneIsKIsAccepted() {
        // flip more bytes of lanes P and D than their Reed–Solomon codes repair (t = 6 and 5);
        // the glyph lane is untouched, and flipping a tile's polarity keeps its glyph
        val encoder = PetalStreamEncoder(streamPayload, 2)
        val clean = encoder.cells(5)
        val p = clean.pWord()
        val d = clean.dWord()
        for (index in 0 until 8) p[4 * index] = (p[4 * index].toInt() xor 0xFF).toByte()
        for (index in 0 until 7) d[4 * index] = (d[4 * index].toInt() xor 0xFF).toByte()
        val luma = PetalRenderer.render(PetalFrameCells.fromWords(p, clean.kWord(), d), PetalRenderOptions(768, 2)).toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "lane K alone is enough to accept an orientation")
        assertEquals("K", decoded.lanes)
        assertContentEquals(encoder.laneData(5).k(), decoded.k?.data)
    }

    @Test
    fun randomWordsAreAlmostNeverAccepted() {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; at seven it let through about
        // one random word in 250 (150 of these 40 000). Lane P stops at six for the same reason.
        // The counts are exact so that this port, fed the same xorshift32 words and byte-valued
        // confidences (many ties, so the ranking must be stable), reproduces the reference decoder
        // bit for bit.
        val rng = PetalXorshift32(0x5EED)
        val trials = 40_000
        for ((lane, expected) in listOf(PetalLane.D to 3, PetalLane.P to 0)) {
            val length = lane.dataLength + lane.parityLength
            var accepted = 0
            repeat(trials) {
                val word = ByteArray(length) { rng.nextByte().toByte() }
                val confidence = DoubleArray(length) { rng.nextByte().toDouble() }
                if (PetalDecoder.decodeWithErasures(lane, word, confidence) != null) accepted += 1
            }
            assertEquals(expected, accepted, "lane ${lane.letter} of $trials random words")
        }
    }

    private fun flip(bytes: ByteArray, position: Int, mask: Int) {
        bytes[position] = (bytes[position].toInt() xor mask).toByte()
    }

    @Test
    fun onlyLaneKUsesTwoThirdsOfItsParityAsErasures() {
        // damaged bytes: `flagged` of them marked least confident, two more hidden. With the extra
        // erasure step of the old schedule the decoder would repair them (2·2 + flagged parity
        // bytes); the capped schedule must refuse instead of risking a wrong codeword.
        for ((lane, flagged) in listOf(PetalLane.D to 7, PetalLane.P to 8)) {
            val data = ByteArray(lane.dataLength) { it.toByte() }
            val word = PetalLanes.encodeLane(lane, data)
            val damaged = word.copyOf()
            val confidence = DoubleArray(word.size) { 1.0 }
            for (position in 0 until flagged) {
                flip(damaged, position, 0xA5)
                confidence[position] = 0.0
            }
            flip(damaged, 20, 0x3C)
            flip(damaged, 21, 0x3C)
            assertNull(PetalDecoder.decodeWithErasures(lane, damaged, confidence), "lane ${lane.letter}")
            // half the parity flagged plus one hidden error stays comfortably repairable
            val repairable = word.copyOf()
            val halfFlagged = DoubleArray(word.size) { 1.0 }
            for (position in 0 until lane.parityLength / 2) {
                flip(repairable, position, 0xA5)
                halfFlagged[position] = 0.0
            }
            flip(repairable, 20, 0x3C)
            val result = assertNotNull(PetalDecoder.decodeWithErasures(lane, repairable, halfFlagged), "lane ${lane.letter}")
            assertContentEquals(data, result.data, "lane ${lane.letter}")
            assertTrue(result.erasures <= lane.parityLength / 2, "lane ${lane.letter}")
        }
        // lane K keeps the two-thirds step: 30 flagged bytes plus 7 hidden errors need it
        // (2·7 + 30 = 44 of 45 parity bytes)
        val data = ByteArray(PetalLanes.K_DATA) { it.toByte() }
        val damaged = PetalLanes.encodeLane(PetalLane.K, data)
        val confidence = DoubleArray(damaged.size) { 1.0 }
        for (position in 0 until 30) {
            flip(damaged, position, 0xA5)
            confidence[position] = 0.0
        }
        for (position in 60 until 67) flip(damaged, position, 0x3C)
        val result = assertNotNull(PetalDecoder.decodeWithErasures(PetalLane.K, damaged, confidence), "30 erasures")
        assertContentEquals(data, result.data)
        assertEquals(30, result.erasures)
    }

    @Test
    fun equalConfidencesAreErasedInPositionOrder() {
        // Every tile the normalised read erases has confidence exactly 0, so ties are the rule, and
        // the ranking must be stable or ports disagree about which bytes are erased. Three damaged
        // bytes at the front plus four hidden ones fit lane D only if exactly the first three
        // positions are erased (3 erasures + 4 errors = all 11 parity bytes): a step that erased
        // the last positions instead would see seven errors.
        val data = ByteArray(PetalLanes.D_DATA) { it.toByte() }
        val word = PetalLanes.encodeLane(PetalLane.D, data)
        for (position in listOf(0, 1, 2, 20, 21, 22, 23)) flip(word, position, 0x5A)
        val confidence = DoubleArray(word.size) { 1.0 }
        val result = assertNotNull(PetalDecoder.decodeWithErasures(PetalLane.D, word, confidence), "ties in order")
        assertContentEquals(data, result.data)
        assertEquals(3, result.erasures)
        assertEquals(7, result.corrected)
    }

    @Test
    fun blankFramesReportNoFinders() {
        assertEquals(PetalDecodeError.NO_FINDERS, PetalDecoder.decode(PetalLuma(320, 240)).error)
    }

    @Test
    fun unusableSizesAreRejectedWithoutWork() {
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, PetalDecoder.decode(PetalLuma(1, 1)).error)
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, PetalDecoder.decode(PetalLuma(47, 400)).error)
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, PetalDecoder.decode(PetalLuma(0, 0)).error)
        val smallBudget = PetalDecodeOptions(maxPixels = 1_000)
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, PetalDecoder.decode(PetalLuma(100, 100), smallBudget).error)
        // just over the default 12,000,000-pixel budget
        assertEquals(PetalDecodeError.UNSUPPORTED_IMAGE, PetalDecoder.decode(PetalLuma(4000, 3001)).error)
        assertNull(PetalDecoder.decodeAt(PetalLuma(8, 8), PetalHomography.IDENTITY))
        // the diagnostics refuse unusable images as well (a PetalLuma cannot carry a mismatched buffer)
        val luma = setup(5).second.toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        assertNull(PetalDecoder.observedCells(PetalLuma(47, 400), decoded))
        assertNull(PetalDecoder.tileMatchError(PetalLuma(47, 400), decoded))
        assertNull(PetalDecoder.observedCells(luma, decoded, smallBudget))
        assertNull(PetalDecoder.tileMatchError(luma, decoded, smallBudget))
        assertNull(PetalDecoder.decodeAt(luma, decoded.homography, smallBudget))
        assertFailsWith<IllegalArgumentException> { PetalDecodeOptions(templateSigmas = doubleArrayOf(0.0, 0.5)) }
        assertFailsWith<IllegalArgumentException> { PetalDecodeOptions(maxPixels = 0) }
    }

    @Test
    fun garbageImagesNeverCrashOrDecode() {
        val rng = PetalXorshift32(99)
        for ((w, h) in listOf(64 to 48, 257 to 129, 320 to 240, 480 to 480)) {
            for (style in 0 until 4) {
                val data = ByteArray(w * h) { i ->
                    when (style) {
                        0 -> rng.nextByte() // white noise
                        1 -> (i % w) * 255 / w // gradient
                        2 -> if ((i / w / 8 + i % w / 8) % 2 == 0) 230 else 20 // checkerboard
                        else -> if ((rng.nextInt().toLong() and 0xFFFF_FFFFL) % 50 == 0L) 255 else 0 // sparse specks
                    }.toByte()
                }
                val result = PetalDecoder.decode(assertNotNull(PetalLuma.fromRaw(w, h, data)))
                assertNull(result.frame, "${w}x$h style $style")
            }
        }
        // degenerate shapes and extreme values must not crash either
        for ((w, h) in listOf(48 to 48, 48 to 2000, 2000 to 48)) {
            assertNotNull(PetalDecoder.decode(PetalLuma(w, h)).error)
            assertNotNull(PetalDecoder.decode(assertNotNull(PetalLuma.fromRaw(w, h, ByteArray(w * h) { -1 }))).error)
        }
    }

    @Test
    fun randomBlobScenesNeverCrashOrYieldLanes() {
        // Scenes with several random bright ellipses (some finder-sized) on noise:
        // exercises the locator, quad selection and homography on degenerate layouts.
        val rng = PetalXorshift32(2024)
        fun next(bound: Int): Int = (PetalTestSupport.u32(rng.nextInt()) % bound).toInt()
        for (scene in 0 until 60) {
            val w = 160 + next(400)
            val h = 120 + next(300)
            val data = ByteArray(w * h) { next(40).toByte() }
            val blobs = 3 + next(8)
            repeat(blobs) {
                val cx = next(w).toDouble()
                val cy = next(h).toDouble()
                val rx = 6.0 + next(40)
                val ry = 6.0 + next(40)
                for (y in 0 until h) {
                    for (x in 0 until w) {
                        val dx = (x - cx) / rx
                        val dy = (y - cy) / ry
                        if (dx * dx + dy * dy <= 1.0) data[y * w + x] = 230.toByte()
                    }
                }
            }
            // must not crash; a lucky layout may locate finders but cannot yield lanes
            val frame = PetalDecoder.decode(assertNotNull(PetalLuma.fromRaw(w, h, data))).frame
            if (frame != null) assertEquals(0, frame.lanesOk, "scene $scene produced lane data from blobs")
        }
    }

    @Test
    fun aValidCodeWithAMissingFinderIsNotMisread() {
        val (_, rgb) = setup(2)
        val n = rgb.width
        val data = rgb.data()
        // erase the bottom-right blossom
        for (y in n * 3 / 4 until n) for (x in n * 3 / 4 until n) for (c in 0 until 3) data[(y * n + x) * 3 + c] = 0
        val damaged = assertNotNull(PetalRgb.fromRaw(n, n, data))
        assertNull(PetalDecoder.decode(damaged.toLuma()).frame)
    }

    @Test
    fun decodedFramesFeedAnAssembler() {
        val encoder = PetalStreamEncoder(streamPayload, 2)
        val assembler = PetalStreamAssembler()
        var frame = 0
        while (assembler.progress().meta == null || !assembler.progress().complete) {
            val luma = PetalTestSupport.renderLuma(encoder, frame, 512, 2)
            val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "frame $frame")
            if (frame == 0) assertEquals(encoder.meta, assertNotNull(decoded.beacon()).meta)
            decoded.feed(assembler)
            frame += 1
            assertTrue(frame <= encoder.systematicFrames, "one systematic pass suffices")
        }
        assertContentEquals(streamPayload, assertNotNull(assembler.takeCompleted()).payload)
    }

    private fun cells(seed: Int): PetalFrameCells {
        val p = ByteArray(PetalLanes.P_DATA) { (it * 31 + seed).toByte() }
        val k = ByteArray(PetalLanes.K_DATA) { ((it * 17) xor seed).toByte() }
        val d = ByteArray(PetalLanes.D_DATA) { ((it * 13) xor seed).toByte() }
        return PetalFrameCells.fromWords(
            PetalLanes.encodeLane(PetalLane.P, p),
            PetalLanes.encodeLane(PetalLane.K, k),
            PetalLanes.encodeLane(PetalLane.D, d),
        )
    }

    @Test
    fun findersAreSolidBlossomsAndCornersAreOtherwiseBlack() {
        val image = PetalRenderer.render(cells(1), PetalRenderOptions(256, 2))
        val at = { x: Int, y: Int -> image.rgbAt(x, y) ushr 16 }
        val scale = 256.0 / 1024.0
        val fx = PetalLayout.finderCenterX(0) * scale
        val fy = PetalLayout.finderCenterY(0) * scale
        assertTrue(at(fx.toInt(), fy.toInt()) > 200, "core must be lit")
        assertTrue(at(fx.toInt(), (fy - 34.0 * scale).toInt()) > 200, "upper petal must be lit")
        assertTrue(at((fx + 20.0 * scale).toInt(), (fy + 20.0 * scale).toInt()) > 200, "blossom body must be lit")
        assertEquals(0, at(2, 255))
    }

    @Test
    fun lightTilesAreBrightAndDarkTilesAreMostlyBlack() {
        val frame = cells(2)
        val image = PetalRenderer.render(frame, PetalRenderOptions(512, 2))
        val scale = 512.0 / 1024.0
        val lightMeans = ArrayList<Double>()
        val darkMeans = ArrayList<Double>()
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val center = PetalLayout.tileCenter(tile)
            val x0 = ((center[0] - 10.0) * scale).toInt()
            val y0 = ((center[1] - 10.0) * scale).toInt()
            var sum = 0
            for (j in 0 until 10) for (i in 0 until 10) sum += image.rgbAt(x0 + i, y0 + j) ushr 16
            (if (frame.isLight(tile)) lightMeans else darkMeans) += sum / 100.0
        }
        val light = lightMeans.average()
        val dark = darkMeans.average()
        // bold glyphs ink the middle of every tile, so only the ordering is stable
        assertTrue(light > dark + 25.0, "light tiles average $light, dark tiles $dark")
    }

    @Test
    fun litDotsAreDrawnAndUnlitSlotsAreBlack() {
        val frame = cells(3)
        val image = PetalRenderer.render(frame, PetalRenderOptions(1024, 1))
        var checked = 0
        for (ring in 0 until PetalLayout.RING_COUNT) {
            for (slot in 0 until PetalLayout.ringSlots(ring)) {
                val center = PetalLayout.slotCenter(ring, slot)
                val value = image.rgbAt(center[0].toInt(), center[1].toInt()) ushr 16
                if (frame.isDotLit(PetalLayout.ringOffset(ring) + slot)) {
                    assertTrue(value > 150, "ring $ring slot $slot should be lit")
                } else {
                    assertEquals(0, value, "ring $ring slot $slot should be dark")
                }
                checked += 1
            }
        }
        assertEquals(PetalLayout.TOTAL_SLOTS, checked)
    }

    @Test
    fun softwareRendersAreByteIdenticalToTheReference() {
        // CRC-32C of the Rust reference `render` output for frames of the `one-pass` fixture stream.
        val encoder = PetalStreamEncoder(PetalTestSupport.payload(700, 2), 2)
        val expected = listOf(
            Triple(0, PetalRenderOptions(512, 2), 0xE02A_6A8E.toInt()),
            Triple(1, PetalRenderOptions(300, 3), 0xF7C1_DC01.toInt()),
            Triple(6, PetalRenderOptions(777, 4), 0x9C08_B851.toInt()),
        )
        for ((frame, options, crc) in expected) {
            val rgb = PetalRenderer.render(encoder.cells(frame), options)
            assertEquals(crc, PetalCrc32c.compute(rgb.data()), "frame $frame at ${options.size} px, ${options.supersample}x")
        }
    }

    @Test
    fun drawListDescribesTheSameFrameAsTheSoftwareRenderer() {
        val frame = cells(4)
        val list = PetalRenderer.drawList(frame)
        assertEquals(4 * (1 + PetalLayout.FINDER_PETALS), list.finderDiscs.size)
        assertEquals(4 * PetalLayout.FINDER_PETALS, list.finderNotches.size)
        assertEquals(PetalLayout.TILE_COUNT, list.tiles.size)
        assertEquals(frame.dots().count { it }, list.dots.size)
        assertEquals(23.0 / 32.0, PetalDrawList.GLYPH_SCALE)
        assertEquals(6.5 * 23.0 / 32.0, PetalDrawList.GLYPH_STROKE_WIDTH)
        val image = PetalRenderer.render(frame, PetalRenderOptions(1024, 1))
        val pink = PetalPalette.DEFAULT.pink
        for (dot in list.dots) assertEquals(pink, image.rgbAt(dot.x.toInt(), dot.y.toInt()), "dot at ${dot.x},${dot.y}")
        val light = PetalPalette.DEFAULT.light
        for (disc in list.finderDiscs) assertEquals(light, image.rgbAt(disc.x.toInt(), disc.y.toInt()))
        for (notch in list.finderNotches) {
            assertEquals(PetalPalette.DEFAULT.background, image.rgbAt(notch.x.toInt(), notch.y.toInt()))
        }
        list.tiles.forEach { tile ->
            assertEquals(frame.isLight(tile.index), tile.light)
            assertEquals(frame.glyph(tile.index), tile.glyph)
            assertEquals(if (tile.light) PetalPalette.DEFAULT.ink else pink, list.glyphColor(tile))
            assertEquals(tile.centerX - 11.5, tile.glyphLeft)
            // a tile corner just inside the rounded square shows the tile fill (or black for dark tiles)
            val corner = image.rgbAt((tile.centerX - 11.8).toInt(), (tile.centerY - 9.0).toInt())
            assertEquals(if (tile.light) light else PetalPalette.DEFAULT.background, corner)
        }
    }
}
