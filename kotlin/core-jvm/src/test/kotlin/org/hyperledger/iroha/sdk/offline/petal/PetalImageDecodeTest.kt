package org.hyperledger.iroha.sdk.offline.petal

import java.nio.ByteBuffer
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
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
        assertNull(PetalPose.fit(p, p, -0.05, 640, 480))
        assertNull(PetalPose.fit(p, p.copyOf(7), 0.0, 640, 480))
    }

    @Test
    fun aFittedPoseTakesItsSourcePointsOntoDistortedImagePoints() {
        val src = doubleArrayOf(72.0, 72.0, 952.0, 72.0, 952.0, 952.0, 72.0, 952.0)
        val dst = doubleArrayOf(380.0, 90.0, 900.0, 110.0, 880.0, 640.0, 360.0, 620.0)
        for (distortion in doubleArrayOf(0.0, -0.08, 0.03)) {
            val pose = assertNotNull(PetalPose.fit(src, dst, distortion, 1280, 720))
            assertEquals(distortion, pose.distortion)
            assertEquals(640.0, pose.centerX)
            assertEquals(360.0, pose.centerY)
            assertEquals(1024.0 * 1024.0, pose.scale2)
            for (point in 0 until 4) {
                val mapped = pose.apply(src[2 * point], src[2 * point + 1])
                assertEquals(dst[2 * point], mapped[0], 1e-3, "$distortion")
                assertEquals(dst[2 * point + 1], mapped[1], 1e-3, "$distortion")
            }
        }
        // barrel distortion pulls a mid-edge point toward the image centre
        val plain = assertNotNull(PetalPose.fit(src, dst, 0.0, 1280, 720))
        val barrel = assertNotNull(PetalPose.fit(src, dst, -0.08, 1280, 720))
        assertTrue(Math.abs(plain.apply(512.0, 72.0)[0] - barrel.apply(512.0, 72.0)[0]) < 5.0)
        assertContentEquals(doubleArrayOf(3.0, 4.0), PetalPose.plain(PetalHomography.IDENTITY).apply(3.0, 4.0))
        assertEquals(plain, PetalPose.fit(src, dst, 0.0, 1280, 720))
        assertTrue(plain != barrel)
    }

    @Test
    fun findsTheFourCornerBlossomsInACleanRender() {
        val encoder = PetalStreamEncoder(ByteArray(200) { 9 }, 1)
        val luma = PetalRenderer.renderSquare(encoder.cells(1), PetalRenderOptions(512, 2)).toLuma()
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
        assertTrue(PetalLocator.locateCandidates(PetalLuma(200, 200)).isEmpty())
        assertFalse(PetalLocator.candidates(PetalLuma(0, 0)).hasNext())
    }

    @Test
    fun threeFindersFormingACornerInferTheFourth() {
        val blob = { x: Double, y: Double -> PetalFinder(x, y, 60.0) }
        // top-left, top-right and bottom-left of a slightly rotated square, plus clutter
        val finders = listOf(blob(100.0, 110.0), blob(540.0, 90.0), blob(120.0, 550.0)) +
            (0 until 5).map { PetalFinder(300.0 + 10.0 * it, 300.0, 14.0) }
        val set = assertNotNull(PetalLocator.selectTriple(finders), "a corner of three")
        val inferred = assertNotNull(set.inferred)
        val fourth = set.corners[inferred]
        assertTrue(Math.abs(fourth.x - 560.0) < 1e-9 && Math.abs(fourth.y - 530.0) < 1e-9, "$fourth")
        assertEquals(2, inferred, "the inferred corner is bottom-right in clockwise order")
        // three blossoms in a row are no corner
        assertNull(PetalLocator.selectTriple(listOf(blob(0.0, 0.0), blob(440.0, 0.0), blob(880.0, 0.0))))
    }

    @Test
    fun aSmallerBlobAtTheInferredCornerCompletesTheQuad() {
        // steep tilt: the far finder is under 0.55 of the largest, but it is where the fourth corner belongs
        val finders = listOf(
            PetalFinder(100.0, 100.0, 64.0),
            PetalFinder(540.0, 100.0, 60.0),
            PetalFinder(100.0, 540.0, 62.0),
            PetalFinder(520.0, 515.0, 30.0),
        )
        val strong = PetalLocator.strongFinders(finders)
        assertEquals(3, strong.size)
        val set = assertNotNull(PetalLocator.selectTriple(strong), "triple")
        val full = assertNotNull(PetalLocator.completeTriple(finders, set.quad, assertNotNull(set.inferred)), "completed")
        assertTrue(full.any { Math.abs(it.x - 520.0) < 1e-9 && Math.abs(it.y - 515.0) < 1e-9 })
    }

    /** The 512-pixel render the reference locator tests use. */
    private fun locatorFrame(): PetalRgb =
        PetalRenderer.renderSquare(PetalStreamEncoder(ByteArray(200) { 9 }, 1).cells(1), PetalRenderOptions(512, 2))

    @Test
    fun aHiddenBlossomYieldsAnInferredCandidate() {
        val rgb = locatorFrame()
        val n = rgb.width
        val data = rgb.data()
        // paint over the bottom-left blossom (centre 36, 476 at this size)
        for (y in 420 until n) for (x in 0 until 92) for (c in 0 until 3) data[(y * n + x) * 3 + c] = 0
        val candidates = PetalLocator.locateCandidates(assertNotNull(PetalRgb.fromRaw(n, n, data)).toLuma())
        val inferred = assertNotNull(candidates.firstOrNull { it.inferred != null }, "an inferred candidate")
        val corner = inferred.corners[inferred.inferred ?: 0]
        assertTrue(Math.abs(corner.x - 36.0) < 4.0 && Math.abs(corner.y - 476.0) < 4.0, "$corner")
    }

    /** A canvas-sized image with finder (lit) and reference-canvas (dark) levels painted where they are sampled. */
    private fun levelCard(levels: List<Pair<Int, Int>>): PetalLuma {
        val luma = PetalLuma(1024, 1024)
        fun paint(cx: Double, cy: Double, radius: Double, value: Int) {
            for (y in (cy - radius).toInt()..(cy + radius).toInt()) {
                for (x in (cx - radius).toInt()..(cx + radius).toInt()) {
                    luma.pixels[y * 1024 + x] = value.toByte()
                }
            }
        }
        for (corner in 0 until 4) {
            val cx = PetalLayout.finderCenterX(corner)
            val cy = PetalLayout.finderCenterY(corner)
            val sx = if (cx < 512.0) 1.0 else -1.0
            val sy = if (cy < 512.0) 1.0 else -1.0
            paint(cx, cy, 30.0, levels[corner].first)
            paint(cx + sx * 100.0, cy, 12.0, levels[corner].second)
            paint(cx, cy + sy * 100.0, 12.0, levels[corner].second)
        }
        return luma
    }

    @Test
    fun anInferredCornerNeedsContrastToo() {
        val h = PetalPose.plain(PetalHomography.IDENTITY)
        // even light: the hidden corner (3) gets levels between the others'
        val even = levelCard(listOf(230 to 30, 220 to 25, 210 to 20, 0 to 0))
        val reference = assertNotNull(PetalDecoder.referenceLevels(even, h, 3), "levels")
        assertTrue(reference.lit[3] - reference.dark[3] >= 12.0)
        // the hidden corner's neighbours disagree (one dim, one veiled): the estimates cross
        val uneven = levelCard(listOf(60 to 45, 250 to 20, 200 to 185, 0 to 0))
        assertNull(PetalDecoder.referenceLevels(uneven, h, 3))
        // with every corner seen, the same light is fine
        val seen = levelCard(listOf(60 to 45, 250 to 20, 200 to 185, 240 to 20))
        assertNotNull(PetalDecoder.referenceLevels(seen, h, PetalDecoder.NO_CORNER))
    }

    @Test
    fun followingFindsAMovedBlossomAndRefusesALostOne() {
        val luma = locatorFrame().toLuma()
        val found = assertNotNull(PetalLocator.follow(luma, PetalFinder(48.0, 27.0, 60.0)), "followed")
        assertTrue(Math.abs(found.x - 36.0) < 1.5 && Math.abs(found.y - 36.0) < 1.5, "$found")
        // nothing bright in the canvas corner gap between a blossom and the rings
        assertNull(PetalLocator.follow(luma, PetalFinder(100.0, 15.0, 30.0)))
        // a finder far outside the image is lost, not an error
        assertNull(PetalLocator.follow(luma, PetalFinder(-1e12, 1e15, 60.0)))
        assertNull(PetalLocator.follow(luma, PetalFinder(Double.NaN, 36.0, 60.0)))
        for ((x, y, size) in listOf(
            Triple(1e300, 36.0, 60.0),
            Triple(-1e300, -1e300, 60.0),
            Triple(36.0, Double.POSITIVE_INFINITY, 60.0),
            Triple(36.0, 36.0, Double.NaN),
        )) {
            assertNull(PetalLocator.follow(luma, PetalFinder(x, y, size)), "($x, $y, $size)")
        }
        // a huge disc just covers the whole image
        PetalLocator.follow(luma, PetalFinder(36.0, 36.0, 1e300))
    }


    /** The payload of the reference decoder tests. */
    private val streamPayload = ByteArray(3000) { i -> ((i.toLong() * 2_654_435_761L) ushr 11).toByte() }

    private fun setup(frame: Int): Pair<PetalStreamEncoder, PetalRgb> {
        val encoder = PetalStreamEncoder(streamPayload, 2)
        return encoder to PetalRenderer.renderSquare(encoder.cells(frame), PetalRenderOptions(768, 2))
    }

    private fun assertAllLanes(decoded: PetalDecodedFrame, truth: PetalFrameLanes, name: String) {
        assertContentEquals(truth.d(), decoded.d?.data, "$name: lane D")
        for (sector in 0 until PetalLayout.SECTORS) {
            assertContentEquals(truth.field(sector), decoded.field(sector)?.data, "$name: field lane $sector")
        }
    }

    @Test
    fun cleanRenderDecodesEveryLane() {
        val (encoder, rgb) = setup(5)
        val luma = rgb.toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame)
        assertAllLanes(decoded, encoder.laneData(5), "clean")
        assertEquals(0, decoded.rotation)
        assertEquals(false, decoded.mirrored)
        assertEquals("D0123", decoded.lanes)
        assertEquals(5, decoded.lanesOk)
        assertEquals(java.lang.Double.doubleToRawLongBits(0.0), java.lang.Double.doubleToRawLongBits(decoded.pose.distortion))
        val again = assertNotNull(PetalDecoder.decodeAt(luma, decoded.pose))
        assertEquals("D0123", again.lanes)
        // frame 5 carries atoms on every lane
        assertNull(decoded.beacon())
        assertEquals(5, decoded.atomPackets().size)
        assertEquals(21, decoded.atomPackets().sumOf { it.atomCount })
    }

    @Test
    fun rotatedAndMirroredRendersDecodeWithTheRightOrientation() {
        val (encoder, rgb) = setup(7)
        val truth = encoder.laneData(7)
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
            assertAllLanes(decoded, truth, name)
        }
        // without mirrored hypotheses a mirrored preview is unreadable
        val strict = PetalDecodeOptions(tryMirrored = false)
        assertEquals(PetalDecodeError.NO_ORIENTATION, PetalDecoder.decode(cases.last().image, strict).error)
    }

    private data class OrientationCase(val name: String, val image: PetalLuma, val rotation: Int, val mirrored: Boolean)

    @Test
    fun theFieldAloneTellsTheOrientationWhenTheRingsAreGone() {
        // black out the ring band: no gates, no lane D; a wrong quarter turn reads
        // permuted petals that no field codeword accepts
        val (encoder, rgb) = setup(9)
        val n = rgb.width
        val luma = rgb.toLuma().data()
        val scale = n / 1024.0
        for (y in 0 until n) {
            for (x in 0 until n) {
                val dx = (x + 0.5) / scale - 512.0
                val dy = (y + 0.5) / scale - 512.0
                val r = Math.sqrt(dx * dx + dy * dy)
                if (r in 392.0..490.0) luma[y * n + x] = 0
            }
        }
        val rotated = ByteArray(n * n)
        for (y in 0 until n) for (x in 0 until n) rotated[y * n + x] = luma[(n - 1 - x) * n + y]
        val decoded = assertNotNull(PetalDecoder.decode(assertNotNull(PetalLuma.fromRaw(n, n, rotated))).frame, "decodes")
        assertNull(decoded.d)
        assertEquals(1, decoded.rotation)
        val truth = encoder.laneData(9)
        for (sector in 0 until PetalLayout.SECTORS) assertContentEquals(truth.field(sector), decoded.field(sector)?.data)
    }

    private fun flip(bytes: ByteArray, position: Int, mask: Int) {
        bytes[position] = (bytes[position].toInt() xor mask).toByte()
    }

    @Test
    fun correctedCountsRewrittenBytesNotJustErasures() {
        val data = ByteArray(PetalLanes.FIELD_DATA) { it.toByte() }
        val lane = PetalLane.F1
        val word = PetalLanes.encodeLane(lane, data)
        for (position in intArrayOf(2, 11, 30, 77)) flip(word, position, 0x5A)
        val confidence = DoubleArray(word.size) { 1.0 }
        val result = assertNotNull(PetalDecoder.decodeWithErasures(lane, word, confidence), "four errors fit")
        assertContentEquals(data, result.data)
        assertEquals(0, result.erasures)
        assertEquals(4, result.corrected)
        // with the damaged bytes flagged as least confident, they become erasures
        val flagged = DoubleArray(word.size) { 1.0 }
        for (position in intArrayOf(2, 11, 30, 77)) flagged[position] = 0.0
        val erased = assertNotNull(PetalDecoder.decodeWithErasures(lane, word, flagged), "decodes")
        assertContentEquals(data, erased.data)
        assertTrue(erased.corrected >= 4)
    }

    @Test
    fun randomWordsAreAlmostNeverAccepted() {
        // Reed–Solomon with erasures can accept a word that is not a transmission. Lane D has only
        // 11 parity bytes, so its schedule stops at five erasures; a field lane has 22 and stops at
        // eleven. The counts are exact so that this port, fed the same xorshift32 words and
        // byte-valued confidences (many ties, so the ranking must be stable), reproduces the
        // reference decoder bit for bit.
        val rng = PetalXorshift32(0x5EED)
        val trials = 40_000
        for ((lane, expected) in listOf(PetalLane.D to 3, PetalLane.F0 to 0)) {
            val length = lane.wordLength
            var accepted = 0
            repeat(trials) {
                val word = ByteArray(length) { rng.nextByte().toByte() }
                val confidence = DoubleArray(length) { rng.nextByte().toDouble() }
                if (PetalDecoder.decodeWithErasures(lane, word, confidence) != null) accepted += 1
            }
            assertEquals(expected, accepted, "lane ${lane.letter} of $trials random words")
        }
    }

    @Test
    fun erasuresStopAtHalfTheParity() {
        // damaged bytes: `flagged` of them marked least confident, two more hidden: beyond half
        // the parity the decoder must refuse instead of risking a wrong codeword
        for ((lane, flagged) in listOf(PetalLane.D to 7, PetalLane.F2 to 16)) {
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
            flip(repairable, 24, 0x3C)
            val result = assertNotNull(PetalDecoder.decodeWithErasures(lane, repairable, halfFlagged), "lane ${lane.letter}")
            assertContentEquals(data, result.data, "lane ${lane.letter}")
            assertTrue(result.erasures <= lane.parityLength / 2, "lane ${lane.letter}")
        }
    }

    @Test
    fun equalConfidencesAreErasedInPositionOrder() {
        // Ties must be broken by position or ports disagree about which bytes are erased.
        // Three damaged bytes at the front plus four hidden ones fit lane D only if exactly the
        // first three positions are erased (3 erasures + 4 errors = all 11 parity bytes).
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
    fun twoMeansSplitsTwoClustersAndSurvivesOne() {
        val values = DoubleArray(100) { if (it < 40) 0.05 else 0.9 }
        val t = PetalDecoder.twoMeans(values, values.size)
        assertTrue(t > 0.3 && t < 0.6, "$t")
        val flat = DoubleArray(10) { 0.4 }
        assertTrue(Math.abs(PetalDecoder.twoMeans(flat, flat.size) - 0.4) < 1e-12)
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
        val identity = PetalPose.plain(PetalHomography.IDENTITY)
        assertNull(PetalDecoder.decodeAt(PetalLuma(8, 8), identity))
        // the diagnostics refuse unusable images as well (a PetalLuma cannot carry a mismatched buffer)
        val luma = setup(5).second.toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        assertNull(PetalDecoder.observedCells(PetalLuma(47, 400), decoded))
        assertNull(PetalDecoder.observedCells(luma, decoded, smallBudget))
        assertNull(PetalDecoder.decodeAt(luma, decoded.pose, smallBudget))
        assertFailsWith<IllegalArgumentException> { PetalDecodeOptions(maxPixels = 0) }
    }

    @Test
    fun observedCellsMatchTheDrawnCellsOfACleanRender() {
        val (encoder, rgb) = setup(3)
        val luma = rgb.toLuma()
        val decoded = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        val observed = assertNotNull(PetalDecoder.observedCells(luma, decoded))
        val drawn = encoder.cells(3)
        assertContentEquals(drawn.dots(), observed.dots())
        assertContentEquals(drawn.petals(), observed.petals())
        // the observed petals are left unturned
        assertEquals(PetalFrameCells.of(drawn.dots(), drawn.petals()), observed)
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
        // exercises the locator, quad selection and pose fitting on degenerate layouts.
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

    /** A 768-pixel render of frame [frame] with the blossom of canonical corner [corner] painted over with background. */
    private fun hiddenBlossom(frame: Int, corner: Int): Pair<PetalStreamEncoder, PetalRgb> {
        val (encoder, rgb) = setup(frame)
        val n = rgb.width
        val scale = n / 1024.0
        val cx = PetalLayout.finderCenterX(corner) * scale
        val cy = PetalLayout.finderCenterY(corner) * scale
        val reach = 75.0 * scale
        val data = rgb.data()
        for (y in 0 until n) {
            for (x in 0 until n) {
                val dx = x + 0.5 - cx
                val dy = y + 0.5 - cy
                if (dx * dx + dy * dy <= reach * reach) for (c in 0 until 3) data[(y * n + x) * 3 + c] = 0
            }
        }
        return encoder to assertNotNull(PetalRgb.fromRaw(n, n, data))
    }

    @Test
    fun aHiddenBlossomIsInferredAndEveryLaneStillReads() {
        for (corner in 0 until 4) {
            val (encoder, rgb) = hiddenBlossom(2, corner)
            val result = PetalDecoder.decode(rgb.toLuma())
            val decoded = assertNotNull(result.frame, "corner $corner: ${result.error}")
            assertEquals(corner, decoded.inferredCorner, "corner $corner")
            assertEquals(0, decoded.rotation, "corner $corner")
            assertEquals(false, decoded.mirrored, "corner $corner")
            assertAllLanes(decoded, encoder.laneData(2), "corner $corner")
        }
    }

    @Test
    fun theInferredCornerIsReportedInCodeCoordinatesWhenMirrored() {
        // hide the top-right blossom of the code, then mirror the picture: the hidden
        // blossom appears top-left in the image but is still corner 1 of the code
        val (encoder, rgb) = hiddenBlossom(3, 1)
        val n = rgb.width
        val source = rgb.data()
        val mirrored = ByteArray(source.size)
        for (y in 0 until n) {
            for (x in 0 until n) {
                System.arraycopy(source, (y * n + (n - 1 - x)) * 3, mirrored, (y * n + x) * 3, 3)
            }
        }
        val decoded = assertNotNull(PetalDecoder.decode(assertNotNull(PetalRgb.fromRaw(n, n, mirrored)).toLuma()).frame)
        assertTrue(decoded.mirrored)
        assertEquals(1, decoded.inferredCorner)
        assertContentEquals(encoder.laneData(3).d(), decoded.d?.data)
    }

    @Test
    fun aLargeHiddenRegionNeverReadsWrongData() {
        // the whole bottom-right quarter is gone: rings and petals with it
        val (encoder, rgb) = setup(2)
        val n = rgb.width
        val data = rgb.data()
        for (y in n * 3 / 4 until n) for (x in n * 3 / 4 until n) for (c in 0 until 3) data[(y * n + x) * 3 + c] = 0
        val truth = encoder.laneData(2)
        val frame = PetalDecoder.decode(assertNotNull(PetalRgb.fromRaw(n, n, data)).toLuma()).frame ?: return
        for (lane in PetalLane.ALL) frame.lane(lane)?.let { assertContentEquals(truth.lane(lane), it.data, "lane $lane") }
    }

    /**
     * The render warped by barrel distortion about the image centre, as a cheap lens does:
     * pixel `p` shows what an undistorted camera saw at `c + (p − c) / (1 + k r²/s²)`.
     */
    private fun barrel(image: PetalLuma, k: Double): PetalLuma {
        val w = image.width
        val h = image.height
        val cx = w / 2.0
        val cy = h / 2.0
        val scale = 0.8 * maxOf(w, h)
        val scale2 = scale * scale
        val out = ByteArray(w * h)
        for (y in 0 until h) {
            for (x in 0 until w) {
                val dx = x + 0.5 - cx
                val dy = y + 0.5 - cy
                // invert p_d = c + u (1 + k |u|²/s²) for the undistorted offset u
                var ux = dx
                var uy = dy
                repeat(8) {
                    val f = 1.0 + k * (ux * ux + uy * uy) / scale2
                    ux = dx / f
                    uy = dy / f
                }
                out[y * w + x] = PetalNumerics.roundHalfAway(image.sample(cx + ux, cy + uy)).toInt().toByte()
            }
        }
        return assertNotNull(PetalLuma.fromRaw(w, h, out))
    }

    /** The four finder centres of an upright code, in canonical order. */
    private fun foundCorners(image: PetalLuma): DoubleArray {
        val quad = assertNotNull(PetalLocator.locate(image), "four finders")
        return DoubleArray(8) { if (it % 2 == 0) quad[it / 2].x else quad[it / 2].y }
    }

    @Test
    fun lensDistortionIsFoundWhenThePlainPoseMissesThePetals() {
        // a small, strongly distorted picture: 480 px of code with the corner blossoms far out
        val (encoder, rgb) = setup(6)
        val luma = rgb.toLuma()
        val pixels = luma.data()
        // downscale 768 → 384 by averaging 2×2 blocks, then frame it in 640×480
        val small = ByteArray(640 * 480)
        for (y in 0 until 384) {
            for (x in 0 until 384) {
                var sum = 0
                for ((i, j) in listOf(0 to 0, 1 to 0, 0 to 1, 1 to 1)) sum += pixels[(2 * y + j) * 768 + 2 * x + i].toInt() and 0xFF
                small[(y + 48) * 640 + x + 128] = ((sum + 2) / 4).toByte()
            }
        }
        val distorted = barrel(assertNotNull(PetalLuma.fromRaw(640, 480, small)), -0.12)
        // the plain pose misses petals at the edge of the field
        val plainPose = assertNotNull(PetalDecoder.fitPose(distorted, foundCorners(distorted), 0.0), "pose")
        val plain = assertNotNull(PetalDecoder.decodeAt(distorted, plainPose), "levels")
        assertTrue(plain.fields.any { it == null }, plain.lanes)
        val decoded = assertNotNull(PetalDecoder.decode(distorted).frame, "decodes")
        assertTrue(decoded.pose.distortion < 0.0, "${decoded.pose.distortion}")
        assertAllLanes(decoded, encoder.laneData(6), "barrel")
    }

    /** Shifts a luma image by whole pixels, filling with black. */
    private fun shifted(image: PetalLuma, dx: Int, dy: Int): PetalLuma {
        val source = image.data()
        val out = ByteArray(source.size)
        for (y in 0 until image.height) {
            for (x in 0 until image.width) {
                val sx = x - dx
                val sy = y - dy
                if (sx >= 0 && sy >= 0 && sx < image.width && sy < image.height) {
                    out[y * image.width + x] = source[sy * image.width + sx]
                }
            }
        }
        return assertNotNull(PetalLuma.fromRaw(image.width, image.height, out))
    }

    /** Places a luma image in the middle of a larger black frame. */
    private fun padded(image: PetalLuma, pad: Int): PetalLuma {
        val width = image.width + 2 * pad
        val out = ByteArray(width * (image.height + 2 * pad))
        val source = image.data()
        for (y in 0 until image.height) System.arraycopy(source, y * image.width, out, (y + pad) * width + pad, image.width)
        return assertNotNull(PetalLuma.fromRaw(width, image.height + 2 * pad, out))
    }

    @Test
    fun trackingFollowsASmallMovementAndGivesUpOnAJump() {
        val (encoder, rgb) = setup(6)
        val luma = padded(rgb.toLuma(), 100)
        val first = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        val followed = assertNotNull(PetalDecoder.track(shifted(luma, 9, -6), first), "tracks a 9 px move")
        assertAllLanes(followed, encoder.laneData(6), "tracked")
        assertNull(followed.inferredCorner)
        assertEquals(first.rotation, followed.rotation)
        // more than a finder diameter: tracking refuses, a full decode is needed
        val jumped = shifted(luma, 95, 0)
        assertNull(PetalDecoder.track(jumped, first))
        assertNotNull(PetalDecoder.decode(jumped).frame)
        // unusable images are refused without work
        assertNull(PetalDecoder.track(PetalLuma(47, 400), first))
        assertNull(PetalDecoder.track(luma, first, PetalDecodeOptions(maxPixels = 1_000)))
        // the code left the picture: nothing to follow in a blank frame or in noise
        assertNull(PetalDecoder.track(PetalLuma(luma.width, luma.height), first))
        val rng = PetalXorshift32(77)
        val noise = assertNotNull(PetalLuma.fromRaw(luma.width, luma.height, ByteArray(luma.width * luma.height) { rng.nextByte().toByte() }))
        assertNull(PetalDecoder.track(noise, first))
        // a smaller frame than the pose expects: the finders fall outside it
        assertNull(PetalDecoder.track(assertNotNull(PetalLuma.fromRaw(200, 150, ByteArray(200 * 150) { 40 })), first))
    }

    private fun withPose(frame: PetalDecodedFrame, pose: PetalPose = frame.pose, inferred: Int? = frame.inferredCorner) =
        PetalDecodedFrame(pose, frame.rotation, frame.mirrored, frame.d, frame.fields, inferred)

    @Test
    fun brokenPosesAreRefusedWithoutCrashing() {
        val (_, rgb) = setup(4)
        val luma = rgb.toLuma()
        val previous = assertNotNull(PetalDecoder.decode(luma).frame, "decodes")
        assertNotNull(PetalDecoder.track(luma, previous))
        // a good pose that names a corner that does not exist
        for (corner in listOf(4, 255, -1)) assertNull(PetalDecoder.track(luma, withPose(previous, inferred = corner)), "corner $corner")
        val nonFinite = listOf(
            PetalHomography(DoubleArray(9) { Double.NaN }),
            PetalHomography(doubleArrayOf(Double.POSITIVE_INFINITY, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)),
        )
        for (broken in nonFinite) assertNull(PetalDecoder.decodeAt(luma, PetalPose.plain(broken)))
        // the last one makes every finder far larger than the image
        val huge = PetalHomography(doubleArrayOf(50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 1.0))
        for (broken in nonFinite + huge) {
            for (inferred in listOf(null, 2)) {
                assertNull(PetalDecoder.track(luma, withPose(previous, PetalPose.plain(broken), inferred)), "$broken inferred $inferred")
            }
        }
    }

    @Test
    fun trackingSurvivesABlossomThatDisappears() {
        val (_, rgb) = setup(4)
        val first = assertNotNull(PetalDecoder.decode(rgb.toLuma()).frame, "decodes")
        // the same code, slightly moved, now with the bottom-left blossom covered
        val (encoder, covered) = hiddenBlossom(4, 3)
        val moved = shifted(covered.toLuma(), -5, 4)
        val followed = assertNotNull(PetalDecoder.track(moved, first), "tracks with three blossoms")
        assertEquals(3, followed.inferredCorner)
        assertContentEquals(encoder.laneData(4).d(), followed.d?.data)
        // the diagnostics use the extrapolated levels of the inferred corner
        assertNotNull(PetalDecoder.observedCells(moved, followed))
    }

    @Test
    fun aBlossomThatReappearsIsSeenAgain() {
        val (_, covered) = hiddenBlossom(4, 3)
        val first = assertNotNull(PetalDecoder.decode(covered.toLuma()).frame, "decodes")
        assertEquals(3, first.inferredCorner)
        // the thumb moves away and the hand moves a little
        val (encoder, rgb) = setup(4)
        val followed = assertNotNull(PetalDecoder.track(shifted(rgb.toLuma(), 4, -3), first), "tracks")
        assertNull(followed.inferredCorner)
        assertContentEquals(encoder.laneData(4).d(), followed.d?.data)
        // still covered: still inferred
        val still = assertNotNull(PetalDecoder.track(shifted(covered.toLuma(), 4, -3), first), "tracks")
        assertEquals(3, still.inferredCorner)
    }

    @Test
    fun mergingOneCaptureReadsTheFieldLanesDecodeRead() {
        val workspace = PetalWorkspace()
        for ((name, image) in listOf("clean" to setup(5).second.toLuma(), "hidden corner" to hiddenBlossom(2, 3).second.toLuma())) {
            val frame = assertNotNull(PetalDecoder.decode(image).frame, name)
            assertTrue(frame.fields.all { it != null }, name)
            val evidence = assertNotNull(PetalDecoder.fieldEvidence(image, frame, PetalDecodeOptions.DEFAULT, null), name)
            assertEquals(frame.fields, PetalDecoder.mergeFields(listOf(evidence), BooleanArray(PetalLayout.SECTORS), workspace).toList(), name)
        }
    }

    @Test
    fun averagingCapturesCancelsTheirIndependentNoise() {
        val (encoder, rgb) = setup(5)
        val truth = encoder.laneData(5)
        val clean = rgb.toLuma()
        val frame = assertNotNull(PetalDecoder.decode(clean).frame)
        val workspace = PetalWorkspace()
        val cleanEvidence = assertNotNull(PetalDecoder.fieldEvidence(clean, frame, PetalDecodeOptions.DEFAULT, null))
        val rng = PetalXorshift32(9)
        // uniform noise of ±0.6 of the lit level per cell: about one bit in eight flips in one
        // capture, far more than a field codeword corrects
        val captures = List(4) {
            PetalDecoder.FieldEvidence(DoubleArray(PetalLayout.FIELD_CELLS) { cell -> cleanEvidence.levels[cell] + (rng.nextByte() - 127.5) / 213.0 })
        }
        for (capture in captures) {
            assertTrue(
                PetalDecoder.mergeFields(listOf(capture), BooleanArray(PetalLayout.SECTORS), workspace).all { it == null },
                "one noisy capture must not read a field lane, or the test proves nothing",
            )
        }
        val merged = PetalDecoder.mergeFields(captures, BooleanArray(PetalLayout.SECTORS), workspace)
        for (sector in 0 until PetalLayout.SECTORS) assertContentEquals(truth.field(sector), merged[sector]?.data, "sector $sector")
        // skipped sectors are not read
        val skipped = PetalDecoder.mergeFields(captures, booleanArrayOf(true, false, true, true), workspace)
        assertTrue(skipped[0] == null && skipped[1] != null && skipped[2] == null && skipped[3] == null)
    }

    @Test
    fun evidenceNeedsAUsableImageAndARealCorner() {
        val image = setup(5).second.toLuma()
        val frame = assertNotNull(PetalDecoder.decode(image).frame)
        val workspace = PetalWorkspace()
        val options = PetalDecodeOptions.DEFAULT
        assertNull(PetalDecoder.fieldEvidence(PetalLuma(20, 20), frame, options, null))
        for (corner in listOf(7, -1)) assertNull(PetalDecoder.fieldEvidence(image, withPose(frame, inferred = corner), options, null))
        assertTrue(PetalDecoder.mergeFields(emptyList(), BooleanArray(PetalLayout.SECTORS), workspace).all { it == null })
        // evidence sampled into a buffer the caller hands over equals freshly sampled evidence
        val fresh = assertNotNull(PetalDecoder.fieldEvidence(image, frame, options, null))
        val reused = DoubleArray(PetalLayout.FIELD_CELLS) { Double.NaN }
        val into = assertNotNull(PetalDecoder.fieldEvidence(image, frame, options, reused))
        assertTrue(into.levels === reused)
        assertContentEquals(fresh.levels, reused)
    }

    @Test
    fun inferredCornersMapToCanonicalIndicesUnderEveryOrientation() {
        // the quad corner that a hypothesis maps canonical corner `i` onto must map back to `i`
        val quad = arrayOf(
            PetalFinder(100.0, 100.0, 60.0),
            PetalFinder(500.0, 120.0, 60.0),
            PetalFinder(480.0, 520.0, 60.0),
            PetalFinder(90.0, 490.0, 60.0),
        )
        val canonical = doubleArrayOf(72.0, 72.0, 952.0, 72.0, 952.0, 952.0, 72.0, 952.0)
        for (mirrored in listOf(false, true)) {
            for (rotation in 0 until 4) {
                for (i in 0 until 4) {
                    val index = if (mirrored) (rotation + 4 - i) % 4 else (i + rotation) % 4
                    assertEquals(i, PetalDecoder.canonicalCorner(index, rotation, mirrored), "rotation $rotation mirrored $mirrored")
                }
            }
        }
        // and `hypotheses` puts quad corners where that mapping says
        val hypothesis = PetalDecoder.hypotheses(quad, true).first { it.rotation == 3 && it.mirrored }
        val pose = assertNotNull(PetalDecoder.fitPose(PetalLuma(640, 640), hypothesis.dst, 0.0))
        for (index in 0 until 4) {
            val i = PetalDecoder.canonicalCorner(index, 3, true)
            val mapped = pose.apply(canonical[2 * i], canonical[2 * i + 1])
            assertEquals(quad[index].x, mapped[0], 1e-6)
            assertEquals(quad[index].y, mapped[1], 1e-6)
        }
        assertEquals(4, PetalDecoder.hypotheses(quad, false).size)
    }

    @Test
    fun decodedFramesFeedAnAssembler() {
        val encoder = PetalStreamEncoder(streamPayload.copyOf(300), 2)
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
        assertContentEquals(streamPayload.copyOf(300), assertNotNull(assembler.takeCompleted()).payload)
    }

    /** Frame cells of the reference renderer tests. */
    private fun cells(seed: Int): PetalFrameCells {
        val d = ByteArray(PetalLanes.D_DATA) { ((it * 13) xor seed).toByte() }
        val fields = List(PetalLayout.SECTORS) { sector ->
            PetalLanes.encodeLane(PetalLane.field(sector), ByteArray(PetalLanes.FIELD_DATA) { ((it * 17 + sector) xor seed).toByte() })
        }
        return PetalFrameCells.fromWords(PetalLanes.encodeLane(PetalLane.D, d), fields)
    }

    @Test
    fun findersAreSolidBlossomsAndCornersAreOtherwiseBlack() {
        val image = PetalRenderer.renderSquare(cells(1), PetalRenderOptions(256, 2))
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
    fun litPetalsAreDrawnInTheirColourAndDarkCellsShowTheBackdrop() {
        val frame = cells(2)
        val image = PetalRenderer.renderSquare(frame, PetalRenderOptions(1024, 1))
        val palette = PetalPalette.DEFAULT
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val center = PetalLayout.cellCenter(cell)
            val x = center[0]
            val y = center[1]
            val expected = when {
                frame.isPetalLit(cell) && PetalLayout.isLightCell(cell) -> palette.light
                frame.isPetalLit(cell) -> palette.pink
                PetalLayout.inMask(Math.floor(x) + 0.5, Math.floor(y) + 0.5) -> palette.backdrop
                else -> palette.background
            }
            assertEquals(expected, image.rgbAt(x.toInt(), y.toInt()), "cell $cell")
        }
    }

    @Test
    fun litDotsAreDrawnAndUnlitSlotsAreBlack() {
        val frame = cells(3)
        val image = PetalRenderer.renderSquare(frame, PetalRenderOptions(1024, 1))
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
        // CRC-32C of the Rust reference `render` output (the whole card) for frames of a 700-byte
        // stream (turned petals and katakana tiles included).
        val encoder = PetalStreamEncoder(PetalTestSupport.payload(700, 2), 2)
        val expected = listOf(
            Triple(0, PetalRenderOptions(512, 2), 0xFBDF_7940.toInt()),
            Triple(1, PetalRenderOptions(300, 3), 0xB52F_B2C1.toInt()),
            Triple(6, PetalRenderOptions(777, 4), 0xCA8B_70B1.toInt()),
        )
        for ((frame, options, crc) in expected) {
            val rgb = PetalRenderer.render(encoder.cells(frame), options)
            assertEquals(PetalRenderer.cardHeightPx(options.size), rgb.height)
            assertEquals(crc, PetalCrc32c.compute(rgb.data()), "frame $frame at ${options.size} px, ${options.supersample}x")
        }
    }

    @Test
    fun drawListDescribesTheSameFrameAsTheSoftwareRenderer() {
        val frame = cells(4)
        val list = PetalRenderer.drawList(frame)
        assertEquals(4 * (1 + PetalLayout.FINDER_PETALS), list.finderDiscs.size)
        assertEquals(4 * PetalLayout.FINDER_PETALS, list.finderNotches.size)
        assertEquals(PetalLayout.MASK.sumOf { row -> row.count { it == '#' } }, list.backdropCells.size)
        assertEquals(frame.petals().count { it }, list.petals.size)
        assertEquals(frame.dots().count { it }, list.dots.size)
        val image = PetalRenderer.renderSquare(frame, PetalRenderOptions(1024, 1))
        val palette = PetalPalette.DEFAULT
        for (dot in list.dots) assertEquals(palette.pink, image.rgbAt(dot.x.toInt(), dot.y.toInt()), "dot at ${dot.x},${dot.y}")
        for (disc in list.finderDiscs) assertEquals(palette.light, image.rgbAt(disc.x.toInt(), disc.y.toInt()))
        for (notch in list.finderNotches) assertEquals(palette.background, image.rgbAt(notch.x.toInt(), notch.y.toInt()))
        var previous = -1
        for (petal in list.petals) {
            assertTrue(petal.cell > previous, "petals in cell order")
            previous = petal.cell
            assertTrue(frame.isPetalLit(petal.cell))
            assertEquals(PetalLayout.isLightCell(petal.cell), petal.light)
            assertEquals(PetalLayout.cellAngle(petal.cell), petal.angle)
            assertContentEquals(PetalLayout.cellCenter(petal.cell), doubleArrayOf(petal.centerX, petal.centerY))
            assertEquals(if (petal.light) palette.light else palette.pink, list.petalColor(petal))
            assertEquals(list.petalColor(petal), image.rgbAt(petal.centerX.toInt(), petal.centerY.toInt()), "petal ${petal.cell}")
        }
        // a backdrop cell is a `#` lattice cell; inside the clip and away from any petal it shows the backdrop
        for (cell in list.backdropCells) {
            assertEquals(PetalLayout.MASK_PITCH, cell.right - cell.left)
            assertEquals(PetalLayout.MASK_PITCH, cell.bottom - cell.top)
            assertTrue(PetalLayout.inMask(cell.left + 1.0, cell.top + 1.0))
        }
        // the dark petals inside the 天 show the backdrop
        val darkLight = (0 until PetalLayout.FIELD_CELLS).filter { !frame.isPetalLit(it) && PetalLayout.isLightCell(it) }
        assertTrue(darkLight.size > 100)
        assertTrue(darkLight.count { image.rgbAt(PetalLayout.cellCenter(it)[0].toInt(), PetalLayout.cellCenter(it)[1].toInt()) == palette.backdrop } > 100)
        assertTrue(list.finderDiscs === PetalRenderer.drawList(cells(5)).finderDiscs, "shapes are shared between frames")
        // the katakana tiles: every tile in order, with the cells' symbols, as the card shows them
        assertEquals(PetalLayout.KANA_TILE_COUNT, list.kanaTiles.size)
        val card = PetalRenderer.render(frame, PetalRenderOptions(1024, 1))
        val top = PetalLayout.SQUARE_TOP.toInt()
        for ((index, tile) in list.kanaTiles.withIndex()) {
            assertEquals(index, tile.tile)
            assertEquals(frame.kanaGlyph(index), tile.glyph)
            assertEquals(frame.isKanaLight(index), tile.light)
            // the pixel at the tile centre is the tile's fill or its glyph
            val u = Math.floor(tile.x + 12.5) + 0.5 - tile.x
            val v = Math.floor(tile.y + 12.5) + 0.5 - tile.y
            val expected = when {
                PetalLayout.kanaInked(tile.glyph, u, v) -> list.glyphColor(tile)
                tile.light -> palette.light
                else -> palette.background
            }
            assertEquals(expected, card.rgbAt(Math.floor(tile.x + 12.5).toInt(), Math.floor(tile.y + 12.5).toInt() + top), "tile $index")
        }
    }

    @Test
    fun katakanaTilesShowTheirPolarityAndGlyph() {
        val symbols = cells(4)
        val glyphs = ByteArray(PetalLayout.KANA_TILE_COUNT) { (it % 16).toByte() }
        val light = BooleanArray(PetalLayout.KANA_TILE_COUNT) { it % 3 != 0 }
        val frame = PetalFrameCells.of(symbols.dots(), symbols.petals(), symbols.turns(), glyphs, light)
        val image = PetalRenderer.render(frame, PetalRenderOptions(1024, 1))
        assertEquals(1668, image.height)
        val palette = PetalPalette.DEFAULT
        for ((index, tile) in PetalLayout.kanaTiles().withIndex()) {
            // the pixel centres of the tile's square, against the definition
            val x0 = Math.ceil(tile.x).toInt()
            val y0 = Math.ceil(tile.y + PetalLayout.SQUARE_TOP).toInt()
            for ((dx, dy) in listOf(3 to 3, 12 to 12, 8 to 17, 20 to 6)) {
                val px = x0 + dx
                val py = y0 + dy
                val u = px + 0.5 - tile.x
                val v = py + 0.5 - PetalLayout.SQUARE_TOP - tile.y
                val inked = PetalLayout.kanaInked(glyphs[index].toInt(), u, v)
                val expected = when {
                    inked && light[index] -> palette.ink
                    light[index] -> palette.light
                    inked -> palette.pink
                    else -> palette.background
                }
                assertEquals(expected, image.rgbAt(px, py), "tile $index at ($dx, $dy)")
            }
        }
    }
}
