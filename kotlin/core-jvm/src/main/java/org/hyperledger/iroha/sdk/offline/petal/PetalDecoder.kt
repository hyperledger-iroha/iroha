package org.hyperledger.iroha.sdk.offline.petal

import java.util.Arrays

/**
 * Decoder tuning. Immutable and thread-safe.
 *
 * @param tryMirrored also try horizontally mirrored images (front-camera previews).
 * @param templateSigmas the five blur widths (in template cells) tried for glyph matching.
 * @param maxPixels largest image (in pixels) the decoder accepts; larger frames
 *   should be downscaled by the caller. Bounds memory and work on hostile input.
 */
class PetalDecodeOptions @JvmOverloads constructor(
    /** Whether horizontally mirrored images are tried too. */
    val tryMirrored: Boolean = true,
    templateSigmas: DoubleArray = doubleArrayOf(0.0, 0.5, 0.8, 1.1, 1.5),
    /** Largest accepted image area in pixels. */
    val maxPixels: Int = 12_000_000,
) {
    private val sigmas = templateSigmas.copyOf()

    init {
        require(sigmas.size == TEMPLATE_SIGMA_COUNT) { "exactly $TEMPLATE_SIGMA_COUNT template sigmas are required" }
        require(sigmas.all { !it.isNaN() && !it.isInfinite() }) { "template sigmas must be finite" }
        require(maxPixels > 0) { "maxPixels must be positive" }
    }

    /** The blur widths tried for glyph matching (a copy). */
    fun templateSigmas(): DoubleArray = sigmas.copyOf()

    /** Expected tile patches per sigma: `32 × 64` values each (polarity-major). */
    internal val patterns: Array<DoubleArray> by lazy {
        Array(TEMPLATE_SIGMA_COUNT) { PetalDecoder.buildPatterns(sigmas[it]) }
    }

    /** [patterns] rescaled by their own contrast, as the normalised tile read compares them. */
    internal val normalisedPatterns: Array<DoubleArray> by lazy {
        Array(TEMPLATE_SIGMA_COUNT) { PetalDecoder.rescalePatterns(patterns[it]) }
    }

    override fun equals(other: Any?): Boolean = other is PetalDecodeOptions && tryMirrored == other.tryMirrored &&
        maxPixels == other.maxPixels && sigmas.contentEquals(other.sigmas)

    override fun hashCode(): Int = 31 * (31 * tryMirrored.hashCode() + maxPixels) + sigmas.contentHashCode()

    companion object {
        /** Number of template blur widths. */
        const val TEMPLATE_SIGMA_COUNT = 5

        /** Mirrored previews tried, sigmas `[0, 0.5, 0.8, 1.1, 1.5]`, at most 12,000,000 pixels. */
        @JvmField
        val DEFAULT = PetalDecodeOptions()
    }
}

/** Why a frame could not be decoded at all. */
enum class PetalDecodeError(
    /** Human-readable description. */
    val description: String,
) {
    /** The image is empty, smaller than 48 pixels on a side, or larger than [PetalDecodeOptions.maxPixels]. */
    UNSUPPORTED_IMAGE("petal image has an unsupported size"),

    /** The four corner finders were not found. */
    NO_FINDERS("petal finders not found"),

    /** Finders were found but no orientation produced a readable lane. */
    NO_ORIENTATION("no petal orientation produced a readable lane"),
}

/** A lane that passed its Reed–Solomon check. */
class PetalLaneResult internal constructor(
    private val bytes: ByteArray,
    /**
     * Byte positions the Reed–Solomon decoder rewrote: the erased bytes plus any
     * errors it found among the others (a measure of how close the lane was to failing).
     */
    val corrected: Int,
    /** Bytes that were passed to the decoder as erasures. */
    val erasures: Int,
) {
    /** Lane data bytes: header and atoms, or the beacon (a copy). */
    val data: ByteArray get() = bytes.copyOf()

    internal val dataView: ByteArray get() = bytes

    override fun equals(other: Any?): Boolean = other is PetalLaneResult && bytes.contentEquals(other.bytes) &&
        corrected == other.corrected && erasures == other.erasures

    override fun hashCode(): Int = 31 * bytes.contentHashCode() + erasures
}

/** Everything read from one camera frame. */
class PetalDecodedFrame internal constructor(
    /** Canvas-to-pixel homography that was used. */
    val homography: PetalHomography,
    /** Orientation: how many quarter turns the code is rotated (`0..3`). */
    val rotation: Int,
    /** Whether the image was mirrored. */
    val mirrored: Boolean,
    /** Lane `P` result. */
    val p: PetalLaneResult?,
    /** Lane `K` result. */
    val k: PetalLaneResult?,
    /** Lane `D` result. */
    val d: PetalLaneResult?,
) {
    /** Number of lanes that decoded. */
    val lanesOk: Int get() = (if (p != null) 1 else 0) + (if (k != null) 1 else 0) + (if (d != null) 1 else 0)

    /** Decoded lanes as letters from `"PKD"`, in that order. */
    val lanes: String
        get() = buildString(3) {
            if (p != null) append('P')
            if (k != null) append('K')
            if (d != null) append('D')
        }

    /** The result of [lane], when it decoded. */
    fun lane(lane: PetalLane): PetalLaneResult? = when (lane) {
        PetalLane.P -> p
        PetalLane.K -> k
        PetalLane.D -> d
    }

    /** What lane `D` carried, when it decoded. */
    fun dLane(): PetalDLane? = d?.let { PetalStream.parseDLane(it.dataView) }

    /** The beacon, when lane `D` decoded on a beacon frame. */
    fun beacon(): PetalBeacon? = dLane()?.beacon

    /** Atom packets from every lane that decoded (`P`, `K`, then a non-beacon `D`). */
    fun atomPackets(): List<PetalAtomPacket> {
        val packets = ArrayList<PetalAtomPacket>(3)
        p?.let { PetalStream.parseAtomLane(PetalLane.P, it.dataView) }?.let { packets += it }
        k?.let { PetalStream.parseAtomLane(PetalLane.K, it.dataView) }?.let { packets += it }
        dLane()?.atoms?.let { packets += it }
        return packets
    }

    /** Offers everything this frame carries to [assembler] (lane `D` first). */
    fun feed(assembler: PetalStreamAssembler) {
        dLane()?.let(assembler::pushDLane)
        p?.let { PetalStream.parseAtomLane(PetalLane.P, it.dataView) }?.let(assembler::pushAtoms)
        k?.let { PetalStream.parseAtomLane(PetalLane.K, it.dataView) }?.let(assembler::pushAtoms)
    }
}

/** The outcome of [PetalDecoder.decode]: exactly one of [frame] and [error] is non-null. */
class PetalDecodeResult internal constructor(
    /** The decoded frame on success. */
    val frame: PetalDecodedFrame?,
    /** Why nothing was decoded, on failure. */
    val error: PetalDecodeError?,
) {
    /** Whether a frame was decoded. */
    val isSuccess: Boolean get() = frame != null
}

/**
 * From a camera luma plane to lane data.
 *
 * The decoder locates the four finders, derives a homography for each
 * orientation hypothesis (four rotations, optionally mirrored), picks the
 * orientation whose ring gates line up (and whose lane `D` codeword checks
 * out), then reads the tiles and dots. Every tile is classified *jointly*: the
 * 8×8 sample patch is compared against the 32 hypotheses (polarity × glyph)
 * and the best match wins, so the katakana and the light/dark bit help each
 * other. Cells the decoder is unsure about become Reed–Solomon erasures.
 *
 * The tile *level read* judges every patch against the light and dark levels
 * measured at the finders. When it leaves lane `P` or `K` unreadable, the
 * *normalised read* is tried: it rescales each patch (and each template) by
 * its own contrast, so over-exposure, veiling light, glare, shadows and
 * gradients cancel out. Only a lane that the level read could not decode is
 * taken from the normalised read, so a frame the level read handles costs
 * nothing extra.
 *
 * All functions are thread-safe. [PetalScanSession] reuses scratch buffers
 * between frames; direct calls allocate their own.
 */
object PetalDecoder {
    private const val PATCH = PetalGlyphs.TEMPLATE_N
    private const val CELLS = PATCH * PATCH
    private const val HYPOTHESES = 2 * PetalGlyphs.GLYPH_COUNT
    private const val MIN_SIDE = 48

    /** Relative level of the glyph ink on a light tile (ink / light fill). */
    private const val INK_ON_LIGHT = 0.04

    /** Relative level of a pink glyph on a dark tile (pink / light fill). */
    private const val PINK_ON_DARK = 0.83

    /** Cells cut from each end of a sorted patch to find its robust darkest and brightest level. */
    private const val PATCH_CUT = CELLS / 10

    /**
     * Tiles whose contrast is below this fraction of the median tile contrast
     * become erasures in the normalised read.
     */
    private const val WEAK_TILE = 0.25

    /** Smallest span (in luma levels) of a camera patch that counts as contrast in the normalised read. */
    private const val PATCH_SPAN_FLOOR = 1.0

    /** Smallest span (relative to the light fill) of a template that counts as contrast in the normalised read. */
    private const val TEMPLATE_SPAN_FLOOR = 0.001

    /** Rescaled cells are clamped to this range, a quarter beyond the robust darkest and brightest levels. */
    private const val RESCALED_MIN = -0.25
    private const val RESCALED_MAX = 1.25

    private const val HALF_GLYPH_BOX = PetalLayout.GLYPH_BOX / 2.0
    private const val PATCH_CELL = PetalLayout.GLYPH_BOX / PATCH

    /** Sub-cell sample offsets, in cells. */
    private val CELL_OFFSET_X = doubleArrayOf(-0.25, 0.25, -0.25, 0.25)
    private val CELL_OFFSET_Y = doubleArrayOf(-0.25, -0.25, 0.25, 0.25)

    /** Directions of the eight finder-body samples (`τ·k/8`). */
    private val BODY_COS = DoubleArray(8)
    private val BODY_SIN = DoubleArray(8)

    /** Canonical finder centres `x0, y0, …` (clockwise from the top-left). */
    private val CANONICAL = DoubleArray(8)

    init {
        for (k in 0 until 8) {
            val angle = PetalLayout.TAU * k.toDouble() / 8.0
            BODY_COS[k] = StrictMath.cos(angle)
            BODY_SIN[k] = StrictMath.sin(angle)
        }
        for (finder in 0 until 4) {
            CANONICAL[2 * finder] = PetalLayout.finderCenterX(finder)
            CANONICAL[2 * finder + 1] = PetalLayout.finderCenterY(finder)
        }
    }

    /**
     * Decodes one camera frame.
     *
     * Returns [PetalDecodeError.UNSUPPORTED_IMAGE] for unusable sizes,
     * [PetalDecodeError.NO_FINDERS] when no code is visible and
     * [PetalDecodeError.NO_ORIENTATION] when no orientation yields a readable lane.
     */
    @JvmStatic
    @JvmOverloads
    fun decode(image: PetalLuma, options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT): PetalDecodeResult =
        decode(image, options, PetalWorkspace())

    /**
     * Reads all lanes with a known canvas-to-pixel [homography] (no finder
     * search), as trackers, refinement passes and qualification tooling do.
     * Returns `null` when the image is unusable or the finder reference levels
     * are too weak.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeAt(
        image: PetalLuma,
        homography: PetalHomography,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): PetalDecodedFrame? {
        if (!supported(image, options)) return null
        val reference = referenceLevels(image, homography.m) ?: return null
        return finish(image, options, 0, false, homography, reference, null, PetalWorkspace())
    }

    /** Builds the cells the decoder believes it saw in [frame] (level read), for diagnostics. */
    @JvmStatic
    @JvmOverloads
    fun observedCells(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): PetalFrameCells? {
        val h = frame.homography.m
        val reference = referenceLevels(image, h) ?: return null
        val workspace = PetalWorkspace()
        samplePatches(image, h, workspace)
        val words = tileWords(readTiles(reference, options, workspace))
        return PetalFrameCells.fromWords(words.p, words.k, readDots(image, h, reference).word)
    }

    /** Mean squared tile-match error of [frame] (level read), a quick image-quality indicator. */
    @JvmStatic
    @JvmOverloads
    fun tileMatchError(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): Double? {
        val h = frame.homography.m
        val reference = referenceLevels(image, h) ?: return null
        val workspace = PetalWorkspace()
        samplePatches(image, h, workspace)
        val reads = readTiles(reference, options, workspace)
        var total = -0.0
        for (tile in 0 until reads.count) total += reads.error[tile]
        return total / reads.count.toDouble()
    }

    internal fun decode(image: PetalLuma, options: PetalDecodeOptions, workspace: PetalWorkspace): PetalDecodeResult {
        if (!supported(image, options)) return PetalDecodeResult(null, PetalDecodeError.UNSUPPORTED_IMAGE)
        val finders = PetalLocator.locate(image, workspace)
            ?: return PetalDecodeResult(null, PetalDecodeError.NO_FINDERS)
        val scored = ArrayList<Hypothesis>(8)
        for (hypothesis in hypotheses(finders, options.tryMirrored)) {
            val reference = referenceLevels(image, hypothesis.homography.m) ?: continue
            hypothesis.reference = reference
            hypothesis.score = gateScore(image, hypothesis.homography.m, reference)
            // Stable descending insertion by score (`sort_by` with `b.total_cmp(a)`).
            var at = scored.size
            while (at > 0 && PetalNumerics.totalCompare(hypothesis.score, scored[at - 1].score) > 0) at -= 1
            scored.add(at, hypothesis)
        }
        // 1. the ring beacon is the cheapest and strongest orientation check
        for (index in 0 until minOf(3, scored.size)) {
            val candidate = scored[index]
            if (candidate.score < 0.2) break
            val reference = candidate.reference!!
            val d = readLaneD(image, candidate.homography.m, reference) ?: continue
            return PetalDecodeResult(
                finish(
                    image, options, candidate.rotation, candidate.mirrored, candidate.homography, reference, d, workspace,
                ),
                null,
            )
        }
        // 2. fall back to the tile lanes under the most promising orientations
        for (index in 0 until minOf(4, scored.size)) {
            val candidate = scored[index]
            val h = candidate.homography.m
            val reference = candidate.reference!!
            samplePatches(image, h, workspace)
            val lanes = readTileLanes(reference, options, workspace)
            if (lanes.p != null || lanes.k != null) {
                val frame = PetalDecodedFrame(
                    candidate.homography,
                    candidate.rotation,
                    candidate.mirrored,
                    lanes.p,
                    lanes.k,
                    readLaneD(image, h, reference),
                )
                return PetalDecodeResult(frame, null)
            }
        }
        return PetalDecodeResult(null, PetalDecodeError.NO_ORIENTATION)
    }

    private fun supported(image: PetalLuma, options: PetalDecodeOptions): Boolean =
        image.width >= MIN_SIDE && image.height >= MIN_SIDE &&
            image.width.toLong() * image.height.toLong() <= options.maxPixels.toLong()

    /** Reads every lane under one orientation; [d] may carry the lane `D` result already computed. */
    private fun finish(
        image: PetalLuma,
        options: PetalDecodeOptions,
        rotation: Int,
        mirrored: Boolean,
        homography: PetalHomography,
        reference: Reference,
        d: PetalLaneResult?,
        workspace: PetalWorkspace,
    ): PetalDecodedFrame {
        val h = homography.m
        val dLane = d ?: readLaneD(image, h, reference)
        samplePatches(image, h, workspace)
        val lanes = readTileLanes(reference, options, workspace)
        return PetalDecodedFrame(homography, rotation, mirrored, lanes.p, lanes.k, dLane)
    }

    /** One orientation hypothesis and, once measured, its reference levels and gate score. */
    private class Hypothesis(val rotation: Int, val mirrored: Boolean, val homography: PetalHomography) {
        var reference: Reference? = null
        var score = 0.0
    }

    private fun hypotheses(finders: Array<PetalFinder>, tryMirrored: Boolean): List<Hypothesis> {
        val out = ArrayList<Hypothesis>(8)
        val dst = DoubleArray(8)
        for (mirrored in booleanArrayOf(false, true)) {
            if (mirrored && !tryMirrored) continue
            for (rotation in 0 until 4) {
                for (i in 0 until 4) {
                    val q = if (mirrored) finders[(rotation + 4 - i) % 4] else finders[(i + rotation) % 4]
                    dst[2 * i] = q.x
                    dst[2 * i + 1] = q.y
                }
                PetalHomography.fromPoints(CANONICAL, dst)?.let { out += Hypothesis(rotation, mirrored, it) }
            }
        }
        return out
    }

    /** Lit (finder body) and dark (background) estimates at the four corners. */
    internal class Reference(val lit: DoubleArray, val dark: DoubleArray) {
        fun litAt(x: Double, y: Double): Double = mix(lit, x, y)

        fun darkAt(x: Double, y: Double): Double = mix(dark, x, y)

        /** Bilinear interpolation over the canvas of the four corner estimates. */
        private fun mix(corners: DoubleArray, x: Double, y: Double): Double {
            val u = (x / 1024.0).coerceIn(0.0, 1.0)
            val v = (y / 1024.0).coerceIn(0.0, 1.0)
            val top = corners[0] * (1.0 - u) + corners[1] * u
            val bottom = corners[3] * (1.0 - u) + corners[2] * u
            return top * (1.0 - v) + bottom * v
        }
    }

    /** Samples [image] at the pixel that canvas point `(x, y)` maps to under [h]. */
    private fun sampleThrough(image: PetalLuma, h: DoubleArray, x: Double, y: Double): Double {
        val w = h[6] * x + h[7] * y + h[8]
        return image.sample((h[0] * x + h[1] * y + h[2]) / w, (h[3] * x + h[4] * y + h[5]) / w)
    }

    private fun dotSamples(image: PetalLuma, h: DoubleArray, x: Double, y: Double, spread: Double): Double {
        var sum = 0.0
        sum += sampleThrough(image, h, x + 0.0, y + 0.0)
        sum += sampleThrough(image, h, x + spread, y + 0.0)
        sum += sampleThrough(image, h, x + -spread, y + 0.0)
        sum += sampleThrough(image, h, x + 0.0, y + spread)
        sum += sampleThrough(image, h, x + 0.0, y + -spread)
        return sum / 5.0
    }

    internal fun referenceLevels(image: PetalLuma, h: DoubleArray): Reference? {
        val lit = DoubleArray(4)
        val dark = DoubleArray(4)
        for (i in 0 until 4) {
            val cx = CANONICAL[2 * i]
            val cy = CANONICAL[2 * i + 1]
            // the blossom is solid out to radius 24 around its centre
            var sum = sampleThrough(image, h, cx, cy)
            for (k in 0 until 8) sum += sampleThrough(image, h, cx + 20.0 * BODY_COS[k], cy + 20.0 * BODY_SIN[k])
            lit[i] = sum / 9.0
            val sx = if (cx < 512.0) 1.0 else -1.0
            val sy = if (cy < 512.0) 1.0 else -1.0
            val a = dotSamples(image, h, cx + sx * 100.0, cy, 5.0)
            val b = dotSamples(image, h, cx, cy + sy * 100.0, 5.0)
            dark[i] = 0.5 * (a + b)
            if (lit[i] - dark[i] < 12.0) return null
        }
        return Reference(lit, dark)
    }

    private fun normalisedDot(image: PetalLuma, h: DoubleArray, reference: Reference, flat: Int): Double {
        val x = PetalLayout.SLOT_CENTER_X[flat]
        val y = PetalLayout.SLOT_CENTER_Y[flat]
        val lit = reference.litAt(x, y)
        val dark = reference.darkAt(x, y)
        return (dotSamples(image, h, x, y, 3.5) - dark) / (lit - dark)
    }

    private fun gateScore(image: PetalLuma, h: DoubleArray, reference: Reference): Double {
        var gates = -0.0
        for (slot in PetalLayout.GATE_SLOTS) gates += normalisedDot(image, h, reference, slot)
        var guards = -0.0
        for (slot in PetalLayout.GUARD_SLOTS) guards += normalisedDot(image, h, reference, slot)
        return gates / PetalLayout.GATE_SLOTS.size.toDouble() - guards / PetalLayout.GUARD_SLOTS.size.toDouble()
    }

    /** A transmitted lane `D` word with per-byte confidence. */
    private class DotWord(val word: ByteArray, val confidence: DoubleArray)

    /** Reads lane `D` with per-ring thresholds from the gates (lit) and guards (dark). */
    private fun readDots(image: PetalLuma, h: DoubleArray, reference: Reference): DotWord {
        val thresholds = doubleArrayOf(0.5, 0.5, 0.5)
        for (ring in 0 until PetalLayout.RING_COUNT) {
            var litSum = -0.0
            var litCount = 0
            for (slot in PetalLayout.GATE_SLOTS) {
                if (PetalLayout.SLOT_RING[slot] != ring) continue
                litSum += normalisedDot(image, h, reference, slot)
                litCount += 1
            }
            var darkSum = -0.0
            var darkCount = 0
            for (slot in PetalLayout.GUARD_SLOTS) {
                if (PetalLayout.SLOT_RING[slot] != ring) continue
                darkSum += normalisedDot(image, h, reference, slot)
                darkCount += 1
            }
            if (litCount > 0 && darkCount > 0) {
                val l = litSum / litCount.toDouble()
                val d = darkSum / darkCount.toDouble()
                if (l - d > 0.2) thresholds[ring] = 0.5 * (l + d)
            }
        }
        val word = ByteArray(PetalLanes.D_WORD)
        val confidence = DoubleArray(PetalLanes.D_WORD) { Double.MAX_VALUE }
        for (bit in 0 until PetalLayout.D_BITS) {
            val slot = PetalLayout.DATA_SLOTS[bit]
            val value = normalisedDot(image, h, reference, slot)
            val threshold = thresholds[PetalLayout.SLOT_RING[slot]]
            if (value > threshold) word[bit / 8] = (word[bit / 8].toInt() or (1 shl (7 - bit % 8))).toByte()
            confidence[bit / 8] = PetalNumerics.min(confidence[bit / 8], Math.abs(value - threshold))
        }
        return DotWord(word, confidence)
    }

    /** Lane `D` under one pose. */
    private fun readLaneD(image: PetalLuma, h: DoubleArray, reference: Reference): PetalLaneResult? {
        val dots = readDots(image, h, reference)
        return decodeWithErasures(PetalLane.D, dots.word, dots.confidence)
    }

    /** Tries Reed–Solomon with growing numbers of erasures, least confident first. */
    internal fun decodeWithErasures(lane: PetalLane, word: ByteArray, confidence: DoubleArray): PetalLaneResult? {
        val n = word.size
        // Stable sort of byte positions by confidence (`sort_by` with `total_cmp`).
        val order = IntArray(n) { it }
        for (i in 1 until n) {
            val position = order[i]
            val key = confidence[position]
            var j = i - 1
            while (j >= 0 && PetalNumerics.totalCompare(confidence[order[j]], key) > 0) {
                order[j + 1] = order[j]
                j -= 1
            }
            order[j + 1] = position
        }
        val nsym = lane.parityLength
        val schedule = intArrayOf(0, nsym / 8, nsym / 4, nsym / 3, nsym / 2, nsym * 2 / 3)
        val trial = ByteArray(n)
        var previous = -1
        for (erasures in schedule) {
            if (erasures == previous) continue // `dedup` of consecutive repeats
            previous = erasures
            word.copyInto(trial)
            // zero the erased bytes so stale values cannot leak through
            for (index in 0 until erasures) trial[order[index]] = 0
            return PetalLanes.decodeLaneCountedOrNull(lane, trial, order, erasures) ?: continue
        }
        return null
    }

    /** Expected normalised patches for one blur width: `pred[(polarity * 16 + glyph) * 64 + cell]`. */
    internal fun buildPatterns(sigma: Double): DoubleArray {
        val kernel = DoubleArray(5) { index ->
            val i = index - 2
            if (sigma < 0.05) {
                if (i == 0) 1.0 else 0.0
            } else {
                StrictMath.exp(-(i * i).toDouble() / (2.0 * sigma * sigma))
            }
        }
        var sum = -0.0
        for (value in kernel) sum += value
        for (index in kernel.indices) kernel[index] = kernel[index] / sum
        val pred = DoubleArray(HYPOTHESES * CELLS)
        val coverage = DoubleArray(CELLS)
        val blurred = DoubleArray(CELLS)
        for (polarity in 0 until 2) {
            for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
                for (cell in 0 until CELLS) {
                    coverage[cell] = PetalGlyphs.TEMPLATES[glyph * CELLS + cell].toDouble() / 255.0
                }
                for (v in 0 until PATCH) {
                    for (u in 0 until PATCH) {
                        var acc = 0.0
                        for (ky in 0 until 5) {
                            for (kx in 0 until 5) {
                                val sx = u + kx - 2
                                val sy = v + ky - 2
                                if (sx in 0 until PATCH && sy in 0 until PATCH) {
                                    acc += kernel[kx] * kernel[ky] * coverage[sy * PATCH + sx]
                                }
                            }
                        }
                        blurred[v * PATCH + u] = acc
                    }
                }
                val base = (polarity * PetalGlyphs.GLYPH_COUNT + glyph) * CELLS
                for (cell in 0 until CELLS) {
                    val ink = blurred[cell]
                    pred[base + cell] = if (polarity == 1) 1.0 - (1.0 - INK_ON_LIGHT) * ink else PINK_ON_DARK * ink
                }
            }
        }
        return pred
    }

    /**
     * Rescales every `64`-cell template of [pred] (polarity × glyph) by its own contrast, the way
     * the normalised read rescales the patches it compares them with.
     */
    internal fun rescalePatterns(pred: DoubleArray): DoubleArray {
        val out = DoubleArray(pred.size)
        val sorted = DoubleArray(CELLS)
        val levels = DoubleArray(2)
        for (hypothesis in 0 until HYPOTHESES) {
            val base = hypothesis * CELLS
            patchLevels(pred, base, sorted, levels)
            rescaleInto(pred, base, levels[0], levels[1], TEMPLATE_SPAN_FLOOR, out)
        }
        return out
    }

    /**
     * The robust darkest and brightest level of the [CELLS] values of [values] from [base]: the
     * values [PATCH_CUT] cells in from either end of the sorted cells. Writes `[low, high]` to
     * [levels]; [sorted] is scratch space of [CELLS] values.
     */
    private fun patchLevels(values: DoubleArray, base: Int, sorted: DoubleArray, levels: DoubleArray) {
        System.arraycopy(values, base, sorted, 0, CELLS)
        Arrays.sort(sorted)
        levels[0] = sorted[PATCH_CUT]
        levels[1] = sorted[CELLS - 1 - PATCH_CUT]
    }

    /** The robust darkest and brightest level `[low, high]` of one patch of [CELLS] values. */
    internal fun patchLevels(values: DoubleArray): DoubleArray {
        require(values.size == CELLS) { "a patch has $CELLS cells" }
        val levels = DoubleArray(2)
        patchLevels(values, 0, DoubleArray(CELLS), levels)
        return levels
    }

    /**
     * Maps the patch of [values] at [base] so that [low] becomes 0 and [high] becomes 1, with
     * [floor] as the smallest span that counts as contrast, clamped to `[-0.25, 1.25]`; the result
     * goes to the same cells of [out].
     */
    private fun rescaleInto(values: DoubleArray, base: Int, low: Double, high: Double, floor: Double, out: DoubleArray) {
        val range = PetalNumerics.max(high - low, floor)
        for (cell in base until base + CELLS) {
            out[cell] = ((values[cell] - low) / range).coerceIn(RESCALED_MIN, RESCALED_MAX)
        }
    }

    /** One patch of [CELLS] values mapped by its own darkest level to 0 and brightest to 1. */
    internal fun rescale(values: DoubleArray, floor: Double): DoubleArray {
        val levels = patchLevels(values)
        val out = DoubleArray(CELLS)
        rescaleInto(values, 0, levels[0], levels[1], floor, out)
        return out
    }

    /**
     * Samples the raw 8×8 luma patch of every tile as captured into `workspace.patches`: each cell
     * is the mean of four bilinear samples at ±¼ cell, in camera luma levels (no reference
     * levels applied).
     */
    internal fun samplePatches(image: PetalLuma, h: DoubleArray, workspace: PetalWorkspace) {
        val patches = workspace.patches
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val cx = PetalLayout.TILE_CENTER_X[tile]
            val cy = PetalLayout.TILE_CENTER_Y[tile]
            val base = tile * CELLS
            for (v in 0 until PATCH) {
                val gy = cy - HALF_GLYPH_BOX + (v + 0.5) * PATCH_CELL
                for (u in 0 until PATCH) {
                    val gx = cx - HALF_GLYPH_BOX + (u + 0.5) * PATCH_CELL
                    var sum = 0.0
                    for (offset in 0 until 4) {
                        sum += sampleThrough(
                            image,
                            h,
                            gx + CELL_OFFSET_X[offset] * PATCH_CELL,
                            gy + CELL_OFFSET_Y[offset] * PATCH_CELL,
                        )
                    }
                    patches[base + v * PATCH + u] = sum / 4.0
                }
            }
        }
    }

    /** The joint polarity + glyph reading of every tile under the best blur width. */
    internal class TileReads(val count: Int) {
        val light = BooleanArray(count)
        val glyph = IntArray(count)
        val polarityMargin = DoubleArray(count)
        val glyphMargin = DoubleArray(count)
        val error = DoubleArray(count)
    }

    /**
     * Picks for every patch of [patches] (`TILE_COUNT × 64` values) the polarity and glyph whose
     * template matches best.
     *
     * The template blur is chosen per frame, by the lowest total error. With [rescaleTemplates] the
     * templates are rescaled like the patches; tiles flagged in [erased] get zero margins, so they are
     * the first the Reed–Solomon decoder treats as erasures.
     */
    private fun classify(
        patches: DoubleArray,
        options: PetalDecodeOptions,
        rescaleTemplates: Boolean,
        erased: BooleanArray,
        workspace: PetalWorkspace,
    ): TileReads {
        val errors = workspace.errors
        var bestTotal = Double.MAX_VALUE
        var bestReads = TileReads(0)
        for (pred in if (rescaleTemplates) options.normalisedPatterns else options.patterns) {
            var total = 0.0
            val reads = TileReads(PetalLayout.TILE_COUNT)
            for (tile in 0 until PetalLayout.TILE_COUNT) {
                val base = tile * CELLS
                for (hypothesis in 0 until HYPOTHESES) {
                    val predBase = hypothesis * CELLS
                    var error = -0.0
                    for (cell in 0 until CELLS) {
                        val difference = patches[base + cell] - pred[predBase + cell]
                        error += difference * difference
                    }
                    errors[hypothesis] = error
                }
                var best = 0
                for (hypothesis in 1 until HYPOTHESES) {
                    if (PetalNumerics.totalCompare(errors[hypothesis], errors[best]) < 0) best = hypothesis
                }
                val polarity = best / PetalGlyphs.GLYPH_COUNT
                val glyph = best % PetalGlyphs.GLYPH_COUNT
                var otherPolarity = Double.MAX_VALUE
                val otherBase = (1 - polarity) * PetalGlyphs.GLYPH_COUNT
                for (g in 0 until PetalGlyphs.GLYPH_COUNT) otherPolarity = PetalNumerics.min(otherPolarity, errors[otherBase + g])
                var otherGlyph = Double.MAX_VALUE
                val sameBase = polarity * PetalGlyphs.GLYPH_COUNT
                for (g in 0 until PetalGlyphs.GLYPH_COUNT) {
                    if (g != glyph) otherGlyph = PetalNumerics.min(otherGlyph, errors[sameBase + g])
                }
                val bestError = errors[best]
                total += bestError
                reads.light[tile] = polarity == 1
                reads.glyph[tile] = glyph
                reads.polarityMargin[tile] = if (erased[tile]) 0.0 else otherPolarity - bestError
                reads.glyphMargin[tile] = if (erased[tile]) 0.0 else otherGlyph - bestError
                reads.error[tile] = bestError
            }
            if (total < bestTotal) {
                bestTotal = total
                bestReads = reads
            }
        }
        return bestReads
    }

    /**
     * The level read of the patches in `workspace.patches`: every cell becomes
     * `(v − dark) / (lit − dark)` with the lit and dark levels measured at the finders and
     * interpolated to the tile, and is compared with the templates as drawn.
     */
    internal fun readTiles(reference: Reference, options: PetalDecodeOptions, workspace: PetalWorkspace): TileReads {
        val patches = workspace.patches
        val levelled = workspace.scaled
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val lit = reference.litAt(PetalLayout.TILE_CENTER_X[tile], PetalLayout.TILE_CENTER_Y[tile])
            val dark = reference.darkAt(PetalLayout.TILE_CENTER_X[tile], PetalLayout.TILE_CENTER_Y[tile])
            val base = tile * CELLS
            for (cell in base until base + CELLS) levelled[cell] = (patches[cell] - dark) / (lit - dark)
        }
        Arrays.fill(workspace.erased, false)
        return classify(levelled, options, false, workspace.erased, workspace)
    }

    /**
     * The normalised read of the patches in `workspace.patches`: every patch and every template is
     * rescaled by its own contrast before they are compared, so the judgement does not depend on
     * absolute levels. Tiles whose contrast is below [WEAK_TILE] times the median contrast are erased.
     */
    internal fun readTilesNormalised(options: PetalDecodeOptions, workspace: PetalWorkspace): TileReads {
        val patches = workspace.patches
        val scaled = workspace.scaled
        val spans = workspace.spans
        val levels = workspace.levels
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val base = tile * CELLS
            patchLevels(patches, base, workspace.sortedCells, levels)
            spans[tile] = levels[1] - levels[0]
            rescaleInto(patches, base, levels[0], levels[1], PATCH_SPAN_FLOOR, scaled)
        }
        System.arraycopy(spans, 0, workspace.sortedSpans, 0, PetalLayout.TILE_COUNT)
        Arrays.sort(workspace.sortedSpans)
        val median = workspace.sortedSpans[PetalLayout.TILE_COUNT / 2]
        for (tile in 0 until PetalLayout.TILE_COUNT) workspace.erased[tile] = spans[tile] < WEAK_TILE * median
        return classify(scaled, options, true, workspace.erased, workspace)
    }

    /** Lanes `P` and `K` read from one set of patches. */
    internal class TileLanes(val p: PetalLaneResult?, val k: PetalLaneResult?)

    /**
     * Lanes `P` and `K` from the patches in `workspace.patches`: the level read first, then, for any
     * lane still unreadable, the normalised read.
     */
    internal fun readTileLanes(reference: Reference, options: PetalDecodeOptions, workspace: PetalWorkspace): TileLanes {
        var words = tileWords(readTiles(reference, options, workspace))
        var p = decodeWithErasures(PetalLane.P, words.p, words.pConfidence)
        var k = decodeWithErasures(PetalLane.K, words.k, words.kConfidence)
        if (p == null || k == null) {
            words = tileWords(readTilesNormalised(options, workspace))
            if (p == null) p = decodeWithErasures(PetalLane.P, words.p, words.pConfidence)
            if (k == null) k = decodeWithErasures(PetalLane.K, words.k, words.kConfidence)
        }
        return TileLanes(p, k)
    }

    /** Lane `P` and `K` words with per-byte confidence. */
    internal class TileWords(val p: ByteArray, val pConfidence: DoubleArray, val k: ByteArray, val kConfidence: DoubleArray)

    internal fun tileWords(reads: TileReads): TileWords {
        val p = ByteArray(PetalLanes.P_WORD)
        val pConfidence = DoubleArray(PetalLanes.P_WORD) { Double.MAX_VALUE }
        val k = ByteArray(PetalLanes.K_WORD)
        val kConfidence = DoubleArray(PetalLanes.K_WORD) { Double.MAX_VALUE }
        for (tile in 0 until reads.count) {
            if (reads.light[tile]) p[tile / 8] = (p[tile / 8].toInt() or (1 shl (7 - tile % 8))).toByte()
            pConfidence[tile / 8] = PetalNumerics.min(pConfidence[tile / 8], reads.polarityMargin[tile])
            val glyph = reads.glyph[tile]
            k[tile / 2] = (k[tile / 2].toInt() or (if (tile % 2 == 0) glyph shl 4 else glyph)).toByte()
            kConfidence[tile / 2] = PetalNumerics.min(
                kConfidence[tile / 2],
                PetalNumerics.min(reads.glyphMargin[tile], reads.polarityMargin[tile]),
            )
        }
        return TileWords(p, pConfidence, k, kConfidence)
    }
}
