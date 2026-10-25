package org.hyperledger.iroha.sdk.offline.petal

import java.util.Arrays

/**
 * Decoder tuning. Immutable and thread-safe.
 *
 * @param tryMirrored also try horizontally mirrored images (front-camera previews).
 * @param maxPixels largest image (in pixels) the decoder accepts; larger frames
 *   should be downscaled by the caller. Bounds memory and work on hostile input.
 */
class PetalDecodeOptions @JvmOverloads constructor(
    /** Whether horizontally mirrored images are tried too. */
    val tryMirrored: Boolean = true,
    /** Largest accepted image area in pixels. */
    val maxPixels: Int = 12_000_000,
) {
    init {
        require(maxPixels > 0) { "maxPixels must be positive" }
    }

    override fun equals(other: Any?): Boolean =
        other is PetalDecodeOptions && tryMirrored == other.tryMirrored && maxPixels == other.maxPixels

    override fun hashCode(): Int = 31 * tryMirrored.hashCode() + maxPixels

    companion object {
        /** Mirrored previews tried, at most 12,000,000 pixels. */
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

    /** No set of corner finders (four, or three forming a corner) was found. */
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
    /** Canvas-to-pixel pose that was used. */
    val pose: PetalPose,
    /** Orientation: how many quarter turns the code is rotated (`0..3`). */
    val rotation: Int,
    /** Whether the image was mirrored. */
    val mirrored: Boolean,
    /** Lane `D` result. */
    val d: PetalLaneResult?,
    fields: List<PetalLaneResult?>,
    /**
     * The corner finder that was hidden (by a finger, a glare or the edge of the
     * frame) and inferred from the other three, as its canonical index: 0 top-left,
     * 1 top-right, 2 bottom-right, 3 bottom-left of the upright code; `null` when
     * all four were seen.
     */
    val inferredCorner: Int?,
) {
    /** Field lane results, sector 0 first: four entries, `null` where a lane did not decode. */
    val fields: List<PetalLaneResult?> = java.util.Collections.unmodifiableList(ArrayList(fields))

    init {
        require(this.fields.size == PetalLayout.SECTORS) { "a frame has ${PetalLayout.SECTORS} field lanes" }
    }

    /** The result of the field lane of sector [sector] (`0..3`), when it decoded. */
    fun field(sector: Int): PetalLaneResult? = fields[sector]

    /** Number of lanes that decoded. */
    val lanesOk: Int get() = (if (d != null) 1 else 0) + fields.count { it != null }

    /** Decoded lanes as letters: `D` and the sector digits `0`–`3`, in that order. */
    val lanes: String
        get() = buildString(PetalLane.ALL.size) {
            for (each in PetalLane.ALL) if (lane(each) != null) append(each.letter)
        }

    /** The result of [lane], when it decoded. */
    fun lane(lane: PetalLane): PetalLaneResult? = if (lane.isField) fields[lane.sector] else d

    /** What lane `D` carried, when it decoded. */
    fun dLane(): PetalDLane? = d?.let { PetalStream.parseDLane(it.dataView) }

    /** The beacon, when lane `D` decoded on a beacon frame. */
    fun beacon(): PetalBeacon? = dLane()?.beacon

    /** Atom packets from every lane that decoded (the field lanes in sector order, then a non-beacon `D`). */
    fun atomPackets(): List<PetalAtomPacket> {
        val packets = ArrayList<PetalAtomPacket>(PetalLane.ALL.size)
        for (lane in PetalLane.FIELDS) fields[lane.sector]?.let { PetalStream.parseAtomLane(lane, it.dataView) }?.let { packets += it }
        dLane()?.atoms?.let { packets += it }
        return packets
    }

    /** Offers everything this frame carries to [assembler] (lane `D` first). */
    fun feed(assembler: PetalStreamAssembler) {
        dLane()?.let(assembler::pushDLane)
        for (lane in PetalLane.FIELDS) {
            fields[lane.sector]?.let { PetalStream.parseAtomLane(lane, it.dataView) }?.let(assembler::pushAtoms)
        }
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
 * The decoder tries the finder candidates of [PetalLocator.candidates] in turn
 * (four seen corners, or three with the fourth inferred and moved to where the
 * dotted rings line up), derives a pose for each orientation hypothesis (four
 * rotations, optionally mirrored), ranks the hypotheses by how well the ring
 * gates line up, then reads the dots and the petal field. Every petal is one
 * bit: its level, normalised by the finder levels, is compared with a threshold
 * computed for its codeword and colour class (the petals inside the `天` are
 * lighter than the pink ones). Bytes the decoder is unsure about become
 * Reed–Solomon erasures.
 *
 * When some field lanes do not decode, the pose is searched for radial lens
 * distortion: a cheap camera's barrel distortion moves the petals by a third of
 * their pitch at the edge of a 480p frame, while the finders that fix the
 * homography sit far outside the field. Field lanes the finder levels leave
 * unread are read again from *local levels*: every petal rescaled by the
 * darkest and brightest petals around it, so shadows, glare, banding and
 * veiling light cancel.
 *
 * After a frame decodes, [track] reads the next ones from its pose without the
 * finder search (what [PetalScanSession] does within 500 ms). The session also
 * averages the field levels of captures of the same shown frame to recover
 * field lanes lost to noise or display banding.
 *
 * All functions are thread-safe. [PetalScanSession] reuses scratch buffers
 * between frames; direct calls allocate their own.
 */
object PetalDecoder {
    private const val MIN_SIDE = 48

    /**
     * Distortion steps tried, relative to the starting pose, when a plain pose leaves field lanes
     * unread; the best is then refined by ±[DISTORTION_REFINE].
     */
    private val DISTORTION_STEPS = doubleArrayOf(-0.03, -0.06, -0.09, -0.12, -0.16, 0.03)

    /** Final refinement step of the distortion search. */
    private const val DISTORTION_REFINE = 0.015

    /** A colour class with fewer cells in a codeword than this takes the codeword's threshold. */
    private const val CLASS_MIN_CELLS = 16

    /** Field cells within this distance of a cell are its neighbourhood for the local read. */
    private const val NEIGHBOURHOOD = 32.0

    /** Cells averaged at each end of a sorted neighbourhood for its local dark and lit levels. */
    private const val NEIGHBOURHOOD_ENDS = 5

    /** Sample spread of a ring dot, in canvas units. */
    private const val DOT_SPREAD = 0.32 * PetalLayout.DOT_RADIUS

    /** Sample spread of a petal, in canvas units. */
    private val PETAL_SPREAD = 0.4 * PetalLayout.PETAL_RADIUS_EXACT

    private const val SECTORS = PetalLayout.SECTORS
    private const val FIELD_BITS = PetalLanes.FIELD_BITS

    /** Directions of the eight finder-body samples (`τ·k/8`). */
    private val BODY_COS = DoubleArray(8)
    private val BODY_SIN = DoubleArray(8)

    /** Canonical finder centres `x0, y0, …` (clockwise from the top-left). */
    private val CANONICAL = DoubleArray(8)

    /** No field lane read. */
    private val NO_FIELDS: List<PetalLaneResult?> = java.util.Collections.unmodifiableList(listOf(null, null, null, null))

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
     * Tries the finder candidates of [PetalLocator.candidates] in order and returns the first
     * that reads. For each, the orientation hypotheses (four quarter turns, optionally mirrored)
     * are ranked by the ring gates; lane `D` is tried under the best three whose gate score is at
     * least 0.2, then the field lanes under the best four.
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
     * Follows a code from the [previous] frame that decoded, without searching the whole image
     * for finders (the most expensive part of [decode]).
     *
     * Each corner finder seen in the previous frame is re-found near where the previous pose puts
     * it (see [PetalLocator.follow]); the mean movement of those predicts the rest. A corner
     * inferred in the previous frame counts as seen again only when its blossom is re-found within
     * a quarter diameter of that prediction (so a thumb beside it does not count). When exactly one
     * corner is missing it is placed at the prediction and refined against the rings like an
     * inferred corner. The orientation and the lens distortion are kept from the previous frame
     * (the distortion is searched again when a field lane does not decode). Returns `null` when the
     * image is unusable, the previous pose is broken (non-finite, or finders larger than the
     * image), two corners are lost or no lane decodes; the caller then runs [decode].
     */
    @JvmStatic
    @JvmOverloads
    fun track(
        image: PetalLuma,
        previous: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): PetalDecodedFrame? = track(image, previous, options, PetalWorkspace())

    /**
     * Reads all lanes with a known canvas-to-pixel [pose] (no finder search, no distortion
     * search), as refinement passes and qualification tooling do. The reference levels come from
     * all four corner finders, so a frame whose corner blossom is hidden is refused here; [decode]
     * and [track] read such frames by inferring that corner. Returns `null` when the image is
     * unusable or the finder reference levels are too weak.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeAt(
        image: PetalLuma,
        pose: PetalPose,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): PetalDecodedFrame? {
        if (!supported(image, options)) return null
        val reference = referenceLevels(image, pose, NO_CORNER) ?: return null
        val workspace = PetalWorkspace()
        val levels = workspace.fieldReads[0].levels
        fieldLevels(image, pose, reference, levels)
        val fields = arrayOfNulls<PetalLaneResult>(SECTORS)
        readFields(levels, 0, fields, workspace)
        return PetalDecodedFrame(pose, 0, false, readLaneD(image, pose, reference), fields.toList(), null)
    }

    /**
     * Builds the cells the decoder believes it saw under [frame]'s pose, for diagnostics: every
     * codeword thresholded as the first read of [decode] thresholds it (no local read). Returns
     * `null` when the image is unusable, the frame names a corner that does not exist or the
     * finder levels under that pose are too weak.
     */
    @JvmStatic
    @JvmOverloads
    fun observedCells(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
    ): PetalFrameCells? {
        val evidence = fieldEvidence(image, frame, options, null) ?: return null
        val reference = referenceLevels(image, frame.pose, frame.inferredCorner ?: NO_CORNER) ?: return null
        val d = readDots(image, frame.pose, reference).word
        val workspace = PetalWorkspace()
        val words = List(SECTORS) { sector ->
            readWord(evidence.levels, sector, 0.0, workspace)
            workspace.wordBytes.copyOf()
        }
        return PetalFrameCells.fromWords(d, words)
    }

    /**
     * What one capture shows of a frame's petal field: the level of every field cell sampled under
     * the capture's decoded pose and normalised by that capture's finder levels (about 27 KB).
     */
    internal class FieldEvidence(val levels: DoubleArray)

    /**
     * The field evidence of [image] under the pose [frame] was decoded with, sampled into [into]
     * when given (a buffer the caller no longer needs) or a new buffer. Returns `null` when the
     * image is unusable, the frame names a corner that does not exist, or the finder levels under
     * that pose are too weak; [into] is untouched then.
     */
    internal fun fieldEvidence(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        options: PetalDecodeOptions,
        into: DoubleArray?,
    ): FieldEvidence? {
        // a frame built by the caller may name a corner that does not exist
        if (!supported(image, options) || frame.inferredCorner?.let { it !in 0..3 } == true) return null
        val reference = referenceLevels(image, frame.pose, frame.inferredCorner ?: NO_CORNER) ?: return null
        val levels = into ?: DoubleArray(PetalLayout.FIELD_CELLS)
        fieldLevels(image, frame.pose, reference, levels)
        return FieldEvidence(levels)
    }

    /**
     * The field lanes from several captures of one shown frame: the cell-by-cell mean of their
     * levels (summed in capture order, then divided by the number of captures), read as one
     * capture's levels are, local read included. Averaging cancels the sensor noise, banding and
     * shake that differ between captures; one evidence reads exactly the field lanes [decode] read
     * from that capture's pose. The caller checks that every evidence comes from captures of the
     * same shown frame: a lane read here names its frame in its own header. Sectors flagged in
     * [skip] are not read.
     */
    internal fun mergeFields(
        evidence: Collection<FieldEvidence>,
        skip: BooleanArray,
        workspace: PetalWorkspace,
    ): Array<PetalLaneResult?> {
        val results = arrayOfNulls<PetalLaneResult>(SECTORS)
        if (evidence.isEmpty()) return results
        val mean = workspace.meanLevels
        Arrays.fill(mean, 0.0)
        for (capture in evidence) {
            val levels = capture.levels
            for (cell in 0 until PetalLayout.FIELD_CELLS) mean[cell] += levels[cell]
        }
        val count = evidence.size.toDouble()
        for (cell in 0 until PetalLayout.FIELD_CELLS) mean[cell] /= count
        var mask = 0
        for (sector in 0 until SECTORS) if (skip[sector]) mask = mask or (1 shl sector)
        readFields(mean, mask, results, workspace)
        return results
    }

    internal fun decode(image: PetalLuma, options: PetalDecodeOptions, workspace: PetalWorkspace): PetalDecodeResult {
        if (!supported(image, options)) return PetalDecodeResult(null, PetalDecodeError.UNSUPPORTED_IMAGE)
        var located = false
        val candidates = PetalLocator.Candidates(image, workspace)
        while (candidates.hasNext()) {
            val set = candidates.next()
            located = true
            decodeCandidate(image, options, set, workspace)?.let { return PetalDecodeResult(it, null) }
        }
        return PetalDecodeResult(null, if (located) PetalDecodeError.NO_ORIENTATION else PetalDecodeError.NO_FINDERS)
    }

    /** Marks "no inferred corner" where a corner index is expected. */
    internal const val NO_CORNER = -1

    /**
     * Reads one finder candidate: refines an inferred corner against the rings, ranks the
     * orientation hypotheses by gate score, tries lane `D` under the best three whose gate score is
     * at least 0.2, then the field lanes under the best four. `null` when none reads.
     */
    internal fun decodeCandidate(
        image: PetalLuma,
        options: PetalDecodeOptions,
        set: PetalFinderSet,
        workspace: PetalWorkspace,
    ): PetalDecodedFrame? {
        val missing = set.inferred ?: NO_CORNER
        val corners = if (missing == NO_CORNER) set.quad else refineInferredCorner(image, set.quad, missing)
        val scored = ArrayList<Hypothesis>(8)
        for (hypothesis in hypotheses(corners, options.tryMirrored)) {
            val pose = fitPose(image, hypothesis.dst, 0.0) ?: continue
            val inferred = if (missing == NO_CORNER) NO_CORNER else canonicalCorner(missing, hypothesis.rotation, hypothesis.mirrored)
            val reference = referenceLevels(image, pose, inferred) ?: continue
            hypothesis.pose = pose
            hypothesis.inferred = inferred
            hypothesis.reference = reference
            hypothesis.gate = gateScore(image, pose, reference)
            // Stable descending insertion by gate score (`sort_by` with `b.0.total_cmp(&a.0)`).
            var at = scored.size
            while (at > 0 && PetalNumerics.totalCompare(hypothesis.gate, scored[at - 1].gate) > 0) at -= 1
            scored.add(at, hypothesis)
        }
        // 1. the ring beacon is the cheapest and strongest orientation check
        for (index in 0 until minOf(3, scored.size)) {
            val candidate = scored[index]
            if (candidate.gate < 0.2) continue
            val d = readLaneD(image, candidate.pose!!, candidate.reference!!) ?: continue
            val read = readFieldSearching(image, candidate.dst, 0.0, candidate.inferred, workspace)
            return PetalDecodedFrame(
                read?.pose ?: candidate.pose!!,
                candidate.rotation,
                candidate.mirrored,
                d,
                read?.fields?.toList() ?: NO_FIELDS,
                candidate.inferredCorner(),
            )
        }
        // 2. fall back to the field lanes under the most promising orientations: a wrong
        //    orientation reads permuted petals, which no field codeword accepts. Strong lens
        //    distortion can hide both lane D and the field from a plain pose, so the distortion
        //    is searched under the best orientation whose gates line up even when its plain
        //    pose reads nothing.
        for (index in 0 until minOf(4, scored.size)) {
            val candidate = scored[index]
            val plain = workspace.fieldReads[0]
            if (!readFieldAt(image, candidate.dst, 0.0, candidate.inferred, plain, workspace)) continue
            val confident = index == 0 && candidate.gate >= 0.2
            // the local read costs a sort per cell: only the best orientation gets it for free
            val read = when {
                plain.decoded == SECTORS -> plain
                plain.decoded == 0 && !confident -> if (index == 0) finish(plain, workspace) else plain
                // the search from distortion 0 starts with exactly this plain read
                else -> searchDistortion(image, candidate.dst, 0.0, candidate.inferred, workspace, plain)
            }
            if (read.decoded == 0) continue
            val pose = read.pose!!
            return PetalDecodedFrame(
                pose,
                candidate.rotation,
                candidate.mirrored,
                readLaneD(image, pose, read.reference!!),
                read.fields.toList(),
                candidate.inferredCorner(),
            )
        }
        return null
    }

    internal fun track(
        image: PetalLuma,
        previous: PetalDecodedFrame,
        options: PetalDecodeOptions,
        workspace: PetalWorkspace,
    ): PetalDecodedFrame? {
        // a frame built by the caller may name a corner that does not exist
        if (!supported(image, options) || previous.inferredCorner?.let { it !in 0..3 } == true) return null
        val p0 = previous.pose
        val expected = Array(4) { corner ->
            val cx = CANONICAL[2 * corner]
            val cy = CANONICAL[2 * corner + 1]
            val centre = p0.apply(cx, cy)
            // the finder's diameter is 120 units: its widest projection along either axis
            val across = span(p0, cx - 60.0, cy, cx + 60.0, cy)
            val down = span(p0, cx, cy - 60.0, cx, cy + 60.0)
            PetalFinder(centre[0], centre[1], PetalNumerics.max(across, down))
        }
        // a broken pose, or one whose finders are larger than the picture, cannot be followed
        val short = minOf(image.width, image.height).toDouble()
        for (finder in expected) {
            val finite = !finder.x.isNaN() && !finder.x.isInfinite() && !finder.y.isNaN() && !finder.y.isInfinite() &&
                !finder.size.isNaN() && !finder.size.isInfinite()
            if (!finite || finder.size > short) return null
        }
        val previouslyInferred = previous.inferredCorner ?: NO_CORNER
        val found = arrayOfNulls<PetalFinder>(4)
        for (corner in 0 until 4) {
            if (corner != previouslyInferred) found[corner] = PetalLocator.follow(image, expected[corner])
        }
        // the mean movement of the corners that were followed predicts the others
        var followed = 0
        var sumX = -0.0
        var sumY = -0.0
        for (corner in 0 until 4) {
            val seen = found[corner] ?: continue
            sumX += seen.x - expected[corner].x
            sumY += seen.y - expected[corner].y
            followed += 1
        }
        if (followed < 3) return null
        val shiftX = sumX / followed.toDouble()
        val shiftY = sumY / followed.toDouble()
        fun predicted(corner: Int): PetalFinder =
            PetalFinder(expected[corner].x + shiftX, expected[corner].y + shiftY, expected[corner].size)
        // a corner that was hidden is seen again only when its blossom is found right where the
        // others say it is (a bright thumb beside it must not count)
        if (previouslyInferred != NO_CORNER) {
            val at = predicted(previouslyInferred)
            val again = PetalLocator.follow(image, at)
            found[previouslyInferred] = again?.takeIf {
                val dx = it.x - at.x
                val dy = it.y - at.y
                Math.sqrt(dx * dx + dy * dy) <= 0.25 * at.size
            }
        }
        var lost = 0
        var missing = NO_CORNER
        for (corner in 0 until 4) {
            if (found[corner] == null) {
                lost += 1
                missing = corner
            }
        }
        if (lost > 1) return null
        if (missing != NO_CORNER) found[missing] = predicted(missing)
        var corners = Array(4) { found[it]!! }
        if (missing != NO_CORNER) corners = refineInferredCorner(image, corners, missing)
        val points = DoubleArray(8)
        for (corner in 0 until 4) {
            points[2 * corner] = corners[corner].x
            points[2 * corner + 1] = corners[corner].y
        }
        val read = readFieldSearching(image, points, p0.distortion, missing, workspace) ?: return null
        val pose = read.pose!!
        val d = readLaneD(image, pose, read.reference!!)
        if (d == null && read.decoded == 0) return null
        return PetalDecodedFrame(
            pose,
            previous.rotation,
            previous.mirrored,
            d,
            read.fields.toList(),
            if (missing == NO_CORNER) null else missing,
        )
    }

    /** Pixel distance between the images of canvas points `(ax, ay)` and `(bx, by)` under [pose]. */
    private fun span(pose: PetalPose, ax: Double, ay: Double, bx: Double, by: Double): Double {
        val a = pose.apply(ax, ay)
        val b = pose.apply(bx, by)
        val dx = a[0] - b[0]
        val dy = a[1] - b[1]
        return Math.sqrt(dx * dx + dy * dy)
    }

    private fun supported(image: PetalLuma, options: PetalDecodeOptions): Boolean =
        image.width >= MIN_SIDE && image.height >= MIN_SIDE &&
            image.width.toLong() * image.height.toLong() <= options.maxPixels.toLong()

    /** One orientation hypothesis: the finder centres in canonical corner order and, once measured, its pose, levels and gate score. */
    internal class Hypothesis(val rotation: Int, val mirrored: Boolean, val dst: DoubleArray) {
        /** Canonical index of the inferred corner under this hypothesis, or [NO_CORNER]. */
        var inferred = NO_CORNER
        var pose: PetalPose? = null
        var reference: Reference? = null
        var gate = 0.0

        fun inferredCorner(): Int? = if (inferred == NO_CORNER) null else inferred
    }

    /**
     * Canonical index of the corner at [index] of a finder quad under the orientation hypothesis
     * ([rotation], [mirrored]).
     */
    internal fun canonicalCorner(index: Int, rotation: Int, mirrored: Boolean): Int =
        if (mirrored) (rotation + 4 - index) % 4 else (index + 4 - rotation) % 4

    /** The pose with lens distortion [distortion] through the four finder centres [dst] (image pixels, canonical order). */
    internal fun fitPose(image: PetalLuma, dst: DoubleArray, distortion: Double): PetalPose? =
        PetalPose.fit(CANONICAL, dst, distortion, image.width, image.height)

    /**
     * The brightness summed over all ring slots under the plain pose that maps the canonical
     * corners onto [corners] (`x0, y0, …` in quad order), or NaN when they admit no pose. The
     * slots form the same set of points under every quarter turn and mirror of the canvas (88,
     * 100 and 108 are multiples of four), so the value does not depend on the orientation.
     */
    private fun ringBrightness(image: PetalLuma, corners: DoubleArray): Double {
        val pose = fitPose(image, corners, 0.0) ?: return Double.NaN
        var sum = -0.0
        for (flat in 0 until PetalLayout.TOTAL_SLOTS) {
            sum += dotSamples(image, pose, PetalLayout.SLOT_CENTER_X[flat], PetalLayout.SLOT_CENTER_Y[flat], DOT_SPREAD)
        }
        return sum
    }

    /**
     * Moves the [inferred] corner of [corners] to where the three dotted rings line up best: a
     * 13 × 13 search in steps of 2 % of the mean leg around the parallelogram estimate, then a
     * 9 × 9 search in steps of 0.5 % around the best point. The rings fix the geometry only; the
     * orientation is decided afterwards by the gates and the lanes.
     */
    internal fun refineInferredCorner(image: PetalLuma, corners: Array<PetalFinder>, inferred: Int): Array<PetalFinder> {
        val points = DoubleArray(8)
        for (corner in 0 until 4) {
            points[2 * corner] = corners[corner].x
            points[2 * corner + 1] = corners[corner].y
        }
        val startX = points[2 * inferred]
        val startY = points[2 * inferred + 1]
        fun distance(other: Int): Double {
            val dx = points[2 * other] - startX
            val dy = points[2 * other + 1] - startY
            return Math.sqrt(dx * dx + dy * dy)
        }
        val leg = 0.5 * (distance((inferred + 1) % 4) + distance((inferred + 3) % 4))
        // best[0] brightness, best[1..2] the point (the search runs twice, so it lives in an array)
        val best = doubleArrayOf(-Double.MAX_VALUE, startX, startY)
        fun search(centreX: Double, centreY: Double, step: Double, reach: Int) {
            for (dy in -reach..reach) {
                for (dx in -reach..reach) {
                    val x = centreX + dx.toDouble() * step
                    val y = centreY + dy.toDouble() * step
                    points[2 * inferred] = x
                    points[2 * inferred + 1] = y
                    val brightness = ringBrightness(image, points)
                    if (brightness > best[0]) {
                        best[0] = brightness
                        best[1] = x
                        best[2] = y
                    }
                }
            }
        }
        search(startX, startY, 0.02 * leg, 6)
        search(best[1], best[2], 0.005 * leg, 4)
        val refined = corners.copyOf()
        refined[inferred] = PetalFinder(best[1], best[2], corners[inferred].size)
        return refined
    }

    /** The four orientation hypotheses (and four mirrored ones), in that order. */
    internal fun hypotheses(finders: Array<PetalFinder>, tryMirrored: Boolean): List<Hypothesis> {
        val out = ArrayList<Hypothesis>(8)
        for (mirrored in booleanArrayOf(false, true)) {
            if (mirrored && !tryMirrored) continue
            for (rotation in 0 until 4) {
                val dst = DoubleArray(8)
                for (i in 0 until 4) {
                    val q = if (mirrored) finders[(rotation + 4 - i) % 4] else finders[(i + rotation) % 4]
                    dst[2 * i] = q.x
                    dst[2 * i + 1] = q.y
                }
                out += Hypothesis(rotation, mirrored, dst)
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

    /** Samples [image] at the pixel that canvas point `(x, y)` maps to under [pose] (no allocation). */
    private fun sampleThrough(image: PetalLuma, pose: PetalPose, x: Double, y: Double): Double {
        val h = pose.homography.m
        val w = h[6] * x + h[7] * y + h[8]
        val ux = (h[0] * x + h[1] * y + h[2]) / w
        val uy = (h[3] * x + h[4] * y + h[5]) / w
        val k = pose.distortion
        if (k == 0.0) return image.sample(ux, uy)
        val dx = ux - pose.centerX
        val dy = uy - pose.centerY
        val factor = 1.0 + k * (dx * dx + dy * dy) / pose.scale2
        return image.sample(pose.centerX + dx * factor, pose.centerY + dy * factor)
    }

    /** The mean of five samples: the centre and four points at ±[spread] on `x`, then on `y`. */
    private fun dotSamples(image: PetalLuma, pose: PetalPose, x: Double, y: Double, spread: Double): Double {
        var sum = 0.0
        sum += sampleThrough(image, pose, x + 0.0, y + 0.0)
        sum += sampleThrough(image, pose, x + spread, y + 0.0)
        sum += sampleThrough(image, pose, x + -spread, y + 0.0)
        sum += sampleThrough(image, pose, x + 0.0, y + spread)
        sum += sampleThrough(image, pose, x + 0.0, y + -spread)
        return sum / 5.0
    }

    /**
     * Light and dark levels at the four corners: the solid blossom core, and the black canvas 100
     * units inward of it; `null` when a seen corner has less than 12 levels of contrast (or a
     * non-finite one, under a broken pose). An [inferred] corner (canonical index, or
     * [NO_CORNER]) was not seen, so its levels are extrapolated from the other three by the
     * parallelogram rule and kept within their range.
     */
    internal fun referenceLevels(image: PetalLuma, pose: PetalPose, inferred: Int): Reference? {
        val lit = DoubleArray(4)
        val dark = DoubleArray(4)
        for (i in 0 until 4) {
            if (i == inferred) continue
            val cx = CANONICAL[2 * i]
            val cy = CANONICAL[2 * i + 1]
            // the blossom is solid out to radius 24 around its centre
            var sum = sampleThrough(image, pose, cx, cy)
            for (k in 0 until 8) sum += sampleThrough(image, pose, cx + 20.0 * BODY_COS[k], cy + 20.0 * BODY_SIN[k])
            lit[i] = sum / 9.0
            val sx = if (cx < 512.0) 1.0 else -1.0
            val sy = if (cy < 512.0) 1.0 else -1.0
            val a = dotSamples(image, pose, cx + sx * 100.0, cy, 5.0)
            val b = dotSamples(image, pose, cx, cy + sy * 100.0, 5.0)
            dark[i] = 0.5 * (a + b)
            // a NaN contrast (from a broken pose) refuses as well
            val contrast = lit[i] - dark[i]
            if (contrast.isNaN() || contrast.isInfinite() || contrast < 12.0) return null
        }
        if (inferred != NO_CORNER) {
            lit[inferred] = extrapolate(lit, inferred)
            dark[inferred] = extrapolate(dark, inferred)
            // uneven light can push the estimates past each other; an inferred corner
            // needs the same contrast as a seen one
            if (lit[inferred] - dark[inferred] < 12.0) return null
        }
        return Reference(lit, dark)
    }

    /** The parallelogram estimate `v[n1] + v[n2] − v[opposite]` for corner [m], clamped to the range of the other three. */
    private fun extrapolate(v: DoubleArray, m: Int): Double {
        val n1 = (m + 1) % 4
        val opposite = (m + 2) % 4
        val n2 = (m + 3) % 4
        val low = PetalNumerics.min(PetalNumerics.min(v[n1], v[opposite]), v[n2])
        val high = PetalNumerics.max(PetalNumerics.max(v[n1], v[opposite]), v[n2])
        val estimate = v[n1] + v[n2] - v[opposite]
        return if (estimate < low) low else if (estimate > high) high else estimate
    }

    private fun normalisedDot(image: PetalLuma, pose: PetalPose, reference: Reference, flat: Int): Double {
        val x = PetalLayout.SLOT_CENTER_X[flat]
        val y = PetalLayout.SLOT_CENTER_Y[flat]
        val lit = reference.litAt(x, y)
        val dark = reference.darkAt(x, y)
        return (dotSamples(image, pose, x, y, DOT_SPREAD) - dark) / (lit - dark)
    }

    /** The mean normalised sample of the gate dots minus that of the guards. */
    internal fun gateScore(image: PetalLuma, pose: PetalPose, reference: Reference): Double {
        var gates = -0.0
        for (slot in PetalLayout.GATE_SLOTS) gates += normalisedDot(image, pose, reference, slot)
        var guards = -0.0
        for (slot in PetalLayout.GUARD_SLOTS) guards += normalisedDot(image, pose, reference, slot)
        return gates / PetalLayout.GATE_SLOTS.size.toDouble() - guards / PetalLayout.GUARD_SLOTS.size.toDouble()
    }

    /** A transmitted lane `D` word with per-byte confidence. */
    private class DotWord(val word: ByteArray, val confidence: DoubleArray)

    /** Reads lane `D` with per-ring thresholds from the gates (lit) and guards (dark). */
    private fun readDots(image: PetalLuma, pose: PetalPose, reference: Reference): DotWord {
        val thresholds = doubleArrayOf(0.5, 0.5, 0.5)
        for (ring in 0 until PetalLayout.RING_COUNT) {
            var litSum = -0.0
            var litCount = 0
            for (slot in PetalLayout.GATE_SLOTS) {
                if (PetalLayout.SLOT_RING[slot] != ring) continue
                litSum += normalisedDot(image, pose, reference, slot)
                litCount += 1
            }
            var darkSum = -0.0
            var darkCount = 0
            for (slot in PetalLayout.GUARD_SLOTS) {
                if (PetalLayout.SLOT_RING[slot] != ring) continue
                darkSum += normalisedDot(image, pose, reference, slot)
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
            val value = normalisedDot(image, pose, reference, slot)
            val threshold = thresholds[PetalLayout.SLOT_RING[slot]]
            if (value > threshold) word[bit / 8] = (word[bit / 8].toInt() or (1 shl (7 - bit % 8))).toByte()
            confidence[bit / 8] = PetalNumerics.min(confidence[bit / 8], Math.abs(value - threshold))
        }
        return DotWord(word, confidence)
    }

    /** Lane `D` under one pose. */
    private fun readLaneD(image: PetalLuma, pose: PetalPose, reference: Reference): PetalLaneResult? {
        val dots = readDots(image, pose, reference)
        return decodeWithErasures(PetalLane.D, dots.word, dots.confidence)
    }

    /**
     * Tries Reed–Solomon with growing numbers of erasures, least confident first (a stable
     * ranking: equal confidences keep position order).
     *
     * The schedule erases 0, ⅛, ¼, ⅓ and ½ of the parity bytes (integer division: lane `D`
     * 0, 1, 2, 3, 5; a field lane 0, 2, 5, 7, 11). Lane `D` has only 11 parity bytes, and a further
     * step leaves so few spare ones that it accepts wrong codewords (at 7 erasures about 0.4 % of
     * random words).
     */
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
        val schedule = intArrayOf(0, nsym / 8, nsym / 4, nsym / 3, nsym / 2)
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

    /**
     * The two-means threshold of the first [count] [values]: start at their mean, then move to the
     * midpoint of the means of the values above and at or below the threshold, at most eight
     * times, stopping when it moves less than 10⁻⁴ or one side is empty. Sums run in the order given.
     */
    internal fun twoMeans(values: DoubleArray, count: Int): Double {
        var total = -0.0
        for (index in 0 until count) total += values[index]
        var threshold = total / count.toDouble()
        for (round in 0 until 8) {
            var low = 0.0
            var lows = 0
            var high = 0.0
            var highs = 0
            for (index in 0 until count) {
                val value = values[index]
                if (value > threshold) {
                    high += value
                    highs += 1
                } else {
                    low += value
                    lows += 1
                }
            }
            if (lows == 0 || highs == 0) break
            val next = 0.5 * (low / lows.toDouble() + high / highs.toDouble())
            val moved = Math.abs(next - threshold)
            threshold = next
            if (moved < 1e-4) break
        }
        return threshold
    }

    /** The level of every field cell under one pose, normalised by the finder levels, into [out]. */
    internal fun fieldLevels(image: PetalLuma, pose: PetalPose, reference: Reference, out: DoubleArray) {
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val x = PetalLayout.CELL_X[cell]
            val y = PetalLayout.CELL_Y[cell]
            val lit = reference.litAt(x, y)
            val dark = reference.darkAt(x, y)
            out[cell] = (dotSamples(image, pose, x, y, PETAL_SPREAD) - dark) / (lit - dark)
        }
    }

    /**
     * The neighbourhood of every field cell: the cells (itself included) whose centres lie within
     * [NEIGHBOURHOOD] units, in index order, as `CELLS[START[c] until START[c + 1]]`. Bands are 11
     * units apart, so only the three bands on either side can hold neighbours. Built on first use.
     */
    private object Neighbourhoods {
        val START = IntArray(PetalLayout.FIELD_CELLS + 1)
        val CELLS: ShortArray
        val LARGEST: Int

        init {
            var cells = ShortArray(PetalLayout.FIELD_CELLS * 32)
            var count = 0
            var largest = 0
            val limit = NEIGHBOURHOOD * NEIGHBOURHOOD
            for (cell in 0 until PetalLayout.FIELD_CELLS) {
                val x = PetalLayout.CELL_X[cell]
                val y = PetalLayout.CELL_Y[cell]
                val band = PetalLayout.CELL_BAND[cell]
                val first = PetalLayout.BAND_OFFSETS[maxOf(band - 3, 0)]
                val last = PetalLayout.BAND_OFFSETS[minOf(band + 4, PetalLayout.BAND_COUNT)]
                START[cell] = count
                for (other in first until last) {
                    val dx = PetalLayout.CELL_X[other] - x
                    val dy = PetalLayout.CELL_Y[other] - y
                    if (dx * dx + dy * dy <= limit) {
                        if (count == cells.size) cells = cells.copyOf(2 * cells.size)
                        cells[count] = other.toShort()
                        count += 1
                    }
                }
                largest = maxOf(largest, count - START[cell])
            }
            START[PetalLayout.FIELD_CELLS] = count
            CELLS = cells.copyOf(count)
            LARGEST = largest
        }
    }

    /**
     * Cell levels rescaled by their neighbourhood, into [out]: the mean of the lowest and of the
     * highest [NEIGHBOURHOOD_ENDS] levels around a cell become 0 and 1 (a span below 0.05 counts as
     * 0.05). About half the petals around any cell are lit, so shadows, glare, banding and veiling
     * light, which the finder levels cannot describe, cancel.
     */
    internal fun localLevels(levels: DoubleArray, out: DoubleArray, workspace: PetalWorkspace) {
        val start = Neighbourhoods.START
        val neighbours = Neighbourhoods.CELLS
        val values = workspace.neighbourValues(Neighbourhoods.LARGEST)
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val from = start[cell]
            val n = start[cell + 1] - from
            // insertion sort by `total_cmp` (equal keys are identical values)
            for (index in 0 until n) {
                val value = levels[neighbours[from + index].toInt()]
                var at = index
                while (at > 0 && PetalNumerics.totalCompare(values[at - 1], value) > 0) {
                    values[at] = values[at - 1]
                    at -= 1
                }
                values[at] = value
            }
            val ends = maxOf(minOf(NEIGHBOURHOOD_ENDS, n / 2), 1)
            var low = -0.0
            for (index in 0 until ends) low += values[index]
            low /= ends.toDouble()
            var high = -0.0
            for (index in n - ends until n) high += values[index]
            high /= ends.toDouble()
            out[cell] = (levels[cell] - low) / PetalNumerics.max(high - low, 0.05)
        }
    }

    /**
     * The field lanes read from normalised cell levels into [out] (cleared first), skipping the
     * sectors whose bit is set in [skip], and how far the cells sit from their thresholds on
     * average. Sectors the levels leave unread are read again from their local levels.
     */
    private fun readFields(levels: DoubleArray, skip: Int, out: Array<PetalLaneResult?>, workspace: PetalWorkspace): Double {
        Arrays.fill(out, null)
        val separation = readSectors(levels, skip, out, workspace)
        var retrySkip = skip
        var unread = false
        for (sector in 0 until SECTORS) {
            if (skip and (1 shl sector) != 0) continue
            if (out[sector] == null) unread = true else retrySkip = retrySkip or (1 shl sector)
        }
        if (unread) {
            val local = workspace.localLevels
            localLevels(levels, local, workspace)
            readSectors(local, retrySkip, out, workspace)
        }
        return separation
    }

    /**
     * One read of the field lanes from cell levels with per-codeword, per-colour thresholds: the
     * result of every sector not flagged in [skip] goes to [out]. Returns the mean distance of
     * the cells read from their thresholds (a pose that lands on the petals separates lit from
     * dark cells best).
     */
    internal fun readSectors(levels: DoubleArray, skip: Int, out: Array<PetalLaneResult?>, workspace: PetalWorkspace): Double {
        var separation = 0.0
        var counted = 0
        for (sector in 0 until SECTORS) {
            if (skip and (1 shl sector) != 0) continue
            separation = readWord(levels, sector, separation, workspace)
            counted += FIELD_BITS
            out[sector] = decodeWithErasures(PetalLane.field(sector), workspace.wordBytes, workspace.wordConfidence)
        }
        return separation / maxOf(counted, 1).toDouble()
    }

    /**
     * The transmitted word of one sector read from cell levels into `workspace.wordBytes`, with
     * each byte's confidence (the smallest margin of its bits) in `workspace.wordConfidence`.
     * Every bit's margin from its threshold is added to the running sum [margins], which is returned.
     */
    private fun readWord(levels: DoubleArray, sector: Int, margins: Double, workspace: PetalWorkspace): Double {
        val base = sector * PetalLayout.SECTOR_CELLS
        val values = workspace.sectorValues
        for (bit in 0 until FIELD_BITS) values[bit] = levels[PetalLayout.SECTOR_ORDER[base + bit]]
        val all = twoMeans(values, FIELD_BITS)
        // the light (`天`) and pink petals are thresholded apart: their lit levels differ
        val lightThreshold = classThreshold(values, base, true, all, workspace)
        val pinkThreshold = classThreshold(values, base, false, all, workspace)
        val bytes = workspace.wordBytes
        val confidence = workspace.wordConfidence
        Arrays.fill(bytes, 0.toByte())
        Arrays.fill(confidence, Double.MAX_VALUE)
        var sum = margins
        for (bit in 0 until FIELD_BITS) {
            val threshold = if (PetalLayout.CELL_LIGHT[PetalLayout.SECTOR_ORDER[base + bit]]) lightThreshold else pinkThreshold
            val value = values[bit]
            if (value > threshold) bytes[bit / 8] = (bytes[bit / 8].toInt() or (1 shl (7 - bit % 8))).toByte()
            val margin = Math.abs(value - threshold)
            sum += margin
            confidence[bit / 8] = PetalNumerics.min(confidence[bit / 8], margin)
        }
        return sum
    }

    /** The two-means threshold of the bit cells of one colour class, or [all] for a class of fewer than [CLASS_MIN_CELLS]. */
    private fun classThreshold(values: DoubleArray, base: Int, light: Boolean, all: Double, workspace: PetalWorkspace): Double {
        val members = workspace.classValues
        var count = 0
        for (bit in 0 until FIELD_BITS) {
            if (PetalLayout.CELL_LIGHT[PetalLayout.SECTOR_ORDER[base + bit]] == light) {
                members[count] = values[bit]
                count += 1
            }
        }
        return if (count < CLASS_MIN_CELLS) all else twoMeans(members, count)
    }

    /** A read of the field under one pose; its [levels] buffer belongs to a workspace. */
    internal class FieldRead(val levels: DoubleArray) {
        var pose: PetalPose? = null
        var reference: Reference? = null
        val fields = arrayOfNulls<PetalLaneResult>(PetalLayout.SECTORS)
        var decoded = 0
        var separation = 0.0

        /** More lanes, then a clearer separation. */
        fun betterThan(other: FieldRead): Boolean =
            decoded > other.decoded || (decoded == other.decoded && separation > other.separation)
    }

    /** Reads the field through the finder centres [dst] under the pose with distortion [distortion] into [into] (no local read). */
    private fun readFieldAt(
        image: PetalLuma,
        dst: DoubleArray,
        distortion: Double,
        inferred: Int,
        into: FieldRead,
        workspace: PetalWorkspace,
    ): Boolean {
        val pose = fitPose(image, dst, distortion) ?: return false
        val reference = referenceLevels(image, pose, inferred) ?: return false
        fieldLevels(image, pose, reference, into.levels)
        into.pose = pose
        into.reference = reference
        Arrays.fill(into.fields, null)
        into.separation = readSectors(into.levels, 0, into.fields, workspace)
        into.decoded = into.fields.count { it != null }
        return true
    }

    /** Reads the sectors [read]'s pose left unread again from their local levels. */
    private fun finish(read: FieldRead, workspace: PetalWorkspace): FieldRead {
        if (read.decoded < SECTORS) {
            var skip = 0
            for (sector in 0 until SECTORS) if (read.fields[sector] != null) skip = skip or (1 shl sector)
            val local = workspace.localLevels
            localLevels(read.levels, local, workspace)
            readSectors(local, skip, read.fields, workspace)
            read.decoded = read.fields.count { it != null }
        }
        return read
    }

    /**
     * Reads the field through the finder centres [dst], starting from lens distortion [start], and
     * searches the distortion while some field lane stays unread: [start] plus each of
     * [DISTORTION_STEPS] in turn, then the best ±[DISTORTION_REFINE]. The pose that decodes the
     * most lanes wins (ties: the clearer separation); the search stops once every lane decodes.
     * Lanes the winning pose still leaves unread are read again from its local levels. `null` when
     * no pose through [dst] has usable finder levels. The result is one of the workspace's two
     * reads, valid until the next read.
     */
    private fun readFieldSearching(
        image: PetalLuma,
        dst: DoubleArray,
        start: Double,
        inferred: Int,
        workspace: PetalWorkspace,
    ): FieldRead? {
        val first = workspace.fieldReads[0]
        if (!readFieldAt(image, dst, start, inferred, first, workspace)) return null
        return searchDistortion(image, dst, start, inferred, workspace, first)
    }

    /** [readFieldSearching] from [first], the read at distortion [start] (one of the workspace's reads). */
    private fun searchDistortion(
        image: PetalLuma,
        dst: DoubleArray,
        start: Double,
        inferred: Int,
        workspace: PetalWorkspace,
        first: FieldRead,
    ): FieldRead {
        val reads = workspace.fieldReads
        var best = first
        if (best.decoded == SECTORS) return best
        var candidate = if (best === reads[0]) reads[1] else reads[0]
        for (step in DISTORTION_STEPS) {
            if (readFieldAt(image, dst, start + step, inferred, candidate, workspace) && candidate.betterThan(best)) {
                val previous = best
                best = candidate
                candidate = previous
                if (best.decoded == SECTORS) return best
            }
        }
        val centre = best.pose!!.distortion
        for (side in 0 until 2) {
            val k = if (side == 0) centre - DISTORTION_REFINE else centre + DISTORTION_REFINE
            if (readFieldAt(image, dst, k, inferred, candidate, workspace) && candidate.betterThan(best)) {
                val previous = best
                best = candidate
                candidate = previous
            }
        }
        return finish(best, workspace)
    }
}
