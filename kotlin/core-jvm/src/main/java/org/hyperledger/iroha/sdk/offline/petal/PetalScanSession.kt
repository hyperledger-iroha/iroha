package org.hyperledger.iroha.sdk.offline.petal

import java.util.ArrayDeque

/** Limits of a scan session. Immutable. */
class PetalScanLimits @JvmOverloads constructor(
    /** Forget a half-received stream after this long without progress. */
    val idleTimeoutMillis: Long = 30_000,
    /** Forget a stream that has not finished this long after it started. */
    val absoluteTimeoutMillis: Long = 180_000,
    /** Assembler memory and size limits. */
    val assembler: PetalAssemblerLimits = PetalAssemblerLimits.DEFAULT,
    /** Image decoder options. */
    val decode: PetalDecodeOptions = PetalDecodeOptions.DEFAULT,
) {
    init {
        require(idleTimeoutMillis >= 0 && absoluteTimeoutMillis >= 0) { "timeouts must not be negative" }
    }

    companion object {
        /** 30 s idle and 180 s absolute timeouts with default assembler and decoder settings. */
        @JvmField
        val DEFAULT = PetalScanLimits()
    }
}

/** Counters for diagnostics and UI hints. */
class PetalScanStats internal constructor(
    /** Camera frames offered. */
    val frames: Long,
    /** Frames in which a code was located, whether or not a lane could be read. */
    val located: Long,
    /** Frames in which at least one lane decoded. */
    val readable: Long,
    /** Lane `D` successes. */
    val laneD: Long,
    /** Field lane successes (up to four per frame). */
    val fieldLanes: Long,
    /** Frames read by tracking the previous pose instead of a full search. */
    val tracked: Long,
    /**
     * Frames read with one corner finder hidden and inferred. When a push raises it, a UI can
     * hint that one corner blossom is hidden (a thumb, a glare, the edge of the frame).
     */
    val inferred: Long,
    /** Merges of two or more captures of one shown frame. */
    val merges: Long,
    /** Field lanes recovered by merging captures. */
    val mergedLanes: Long,
    /** Turn lanes tried (each at most once per shown frame). */
    val turnTries: Long,
    /** Turn lanes read. */
    val turnLanes: Long,
    /** Bands tried (each at most once per shown frame). */
    val kanaTries: Long,
    /** Kana lanes read (up to four per shown frame). */
    val kanaLanes: Long,
    /** Captures whose bands read a lane that also tried the turn lanes, having time to spare. */
    val spareTurns: Long,
) {
    override fun equals(other: Any?): Boolean = other is PetalScanStats && frames == other.frames &&
        located == other.located && readable == other.readable && laneD == other.laneD &&
        fieldLanes == other.fieldLanes && tracked == other.tracked && inferred == other.inferred &&
        merges == other.merges && mergedLanes == other.mergedLanes && turnTries == other.turnTries &&
        turnLanes == other.turnLanes && kanaTries == other.kanaTries && kanaLanes == other.kanaLanes &&
        spareTurns == other.spareTurns

    override fun hashCode(): Int =
        (31 * frames + located + 7 * readable + 11 * laneD + 13 * fieldLanes + 17 * tracked + 19 * inferred +
            23 * merges + 29 * mergedLanes + 41 * turnTries + 37 * turnLanes + 43 * kanaTries + 47 * kanaLanes +
            53 * spareTurns).hashCode()

    override fun toString(): String =
        "PetalScanStats(frames=$frames, located=$located, readable=$readable, laneD=$laneD, fieldLanes=$fieldLanes, " +
            "tracked=$tracked, inferred=$inferred, merges=$merges, mergedLanes=$mergedLanes, " +
            "turnTries=$turnTries, turnLanes=$turnLanes, kanaTries=$kanaTries, kanaLanes=$kanaLanes, " +
            "spareTurns=$spareTurns)"
}

/**
 * A part of a capture's work, as a [PetalScanSession] times it to tell whether a capture has time to
 * spare for the turn lanes (see the session's docs).
 */
sealed interface PetalWork {
    /** Finding the code by a full search and taking in the lanes read. */
    data object Search : PetalWork

    /** Following the code from the previous capture's pose and taking in the lanes read. */
    data object Track : PetalWork

    /** Merging the held captures of a shown frame. */
    data object Merge : PetalWork

    /** Reading the kana lanes of [count] bands. */
    data class Bands(val count: Int) : PetalWork

    /** Reading the turn lanes. */
    data object Turns : PetalWork
}

/** What a part of a capture costs a [PetalScanSession], in milliseconds. */
fun interface PetalWorkCost {
    /** What [work] cost, given the milliseconds it took. */
    fun cost(work: PetalWork, measuredMillis: Double): Double

    companion object {
        /** The measured time, as a session on a camera judges its spare time. */
        @JvmField
        val MEASURED = PetalWorkCost { _, measuredMillis -> measuredMillis }
    }
}

/** The result of offering one camera frame. */
class PetalScanOutcome internal constructor(
    /**
     * Why the frame produced nothing, when it did not. [PetalDecodeError.NO_ORIENTATION]
     * means a code was located but no lane could be read (too far, too blurry).
     */
    val error: PetalDecodeError?,
    /** Lanes that decoded in this capture alone, as letters: `D` and the sector digits `0`–`3`. */
    val lanes: String,
    /** Field lanes recovered by merging this capture with earlier captures of its shown frame, as sector digits. */
    val merged: String,
    /** Turn lanes read from this capture, as the sector digits of the field lanes they ride on. */
    val turned: String,
    /** Kana lanes read from this capture, as their names (`P0`, `K0`, `P1`, `K1`). */
    val kana: String,
    /** Receive progress after this frame. */
    val progress: PetalProgress,
    /** The finished payload, delivered exactly once. */
    val completed: PetalCompleted?,
)

/**
 * The receive-side object an app holds while its camera is open: decodes
 * camera frames and reassembles the stream they carry.
 *
 * After a frame decodes, the next frames are first read by
 * [tracking][PetalDecoder.track] the code from its last pose, which skips the
 * finder search; a full [decode][PetalDecoder.decode] runs when tracking fails
 * or the last pose is older than [TRACK_WINDOW_MILLIS]. A half-received stream
 * is forgotten after [PetalScanLimits.idleTimeoutMillis] without progress or
 * [PetalScanLimits.absoluteTimeoutMillis] after it started. Scratch buffers are
 * reused between frames.
 *
 * A camera sees each shown frame two to four times, and noise, display banding or hand shake can
 * cost every one of those captures a field lane. When a capture reads some lane but not every
 * field lane, the session keeps its field evidence (the level of every petal under the capture's
 * pose) under the shown frame its decoded lane headers name, up to [MERGE_CAPTURES] captures of
 * each of the last [MERGE_WINDOW] shown frames. From the second such capture on, it averages them
 * and offers a recovered lane only when that lane's own header names the same frame.
 *
 * Every field lane a capture decodes also lets the session read the turn lane on top of it
 * ([PetalTurns.readTurns]). A turn lane is tried once per shown frame: the session remembers the
 * last [TURN_MEMORY] it tried (cleared with the held evidence when the stream changes) and does not
 * look at those petals again. Reading turns costs a slow phone core several times a frame's decode,
 * so after a try that reads no turn lane (a camera too soft or too shaky for the turns) the session
 * passes over the next captures that would try one, one more after every such try, at most
 * [TURN_BACKOFF], until a try reads a lane again.
 *
 * The katakana bands above and below the square are read the same way ([PetalKana.readKana]):
 * each band once per shown frame (the last [KANA_MEMORY] tried are remembered), with its own
 * back-off of at most [KANA_BACKOFF] captures after tries that read no kana lane (a camera that does
 * not see the bands, or too blurred for their glyphs and polarity).
 *
 * The bands go first. A capture whose bands read a kana lane tries the turn lanes too only when it
 * has time to spare: when the work it has done (finding or following the code, a merge, the bands)
 * and what the session's last turn try took fit together in [CAPTURE_BUDGET_MILLIS], a 30 fps
 * camera's frame time (before the first turn try, twice the work done stands in for it). Otherwise
 * the turn lanes of that shown frame wait for its next capture. On a slow phone core a turn try costs
 * five tracked decodes and a band try one, so reading both on one capture overran the time a frame is
 * shown at 12 fps and the session missed the next shown frames; a fast decoder reads both at once,
 * which matters when its camera gives it few frames a second. A band try that reads nothing (bands
 * out of view, too blurred, or a torn capture) leaves the turn lanes to the same capture.
 *
 * The session times its work on the monotonic clock; a [PetalWorkCost] given to the constructor puts
 * a model's costs in place of the measured times, as tests do.
 *
 * Thread-safe: calls are serialised, so a camera analyzer thread may push while a UI thread reads.
 */
class PetalScanSession @JvmOverloads constructor(
    /** Session limits. */
    val limits: PetalScanLimits = PetalScanLimits.DEFAULT,
    /** What a part of a capture costs, given the milliseconds it took. */
    private val workCost: PetalWorkCost = PetalWorkCost.MEASURED,
) {
    private val assembler = PetalStreamAssembler(limits.assembler)
    private val workspace = PetalWorkspace()
    private var startedMillis: Long? = null
    private var progressMillis = 0L
    private var lastRank = 0
    private var lastPose: PetalDecodedFrame? = null
    private var lastPoseMillis = 0L
    private var frames = 0L
    private var located = 0L
    private var readable = 0L
    private var laneD = 0L
    private var fieldLanes = 0L
    private var tracked = 0L
    private var inferred = 0L
    private var merges = 0L
    private var mergedLanes = 0L
    private var turnTries = 0L
    private var turnLanes = 0L
    private var kanaTries = 0L
    private var kanaLanes = 0L
    private var spareTurns = 0L

    /** What the last turn try cost, in milliseconds. */
    private var turnCostMillis: Double? = null

    /** Captures of one shown frame; bounded by [MERGE_CAPTURES]. */
    internal class Held(val shown: PetalLaneHeader) {
        val evidence = ArrayDeque<PetalDecoder.FieldEvidence>()

        /** The field lanes of this shown frame read so far, by sector. */
        val done = BooleanArray(PetalLayout.SECTORS)

        val complete: Boolean get() = done.all { it }
    }

    internal val held = ArrayDeque<Held>()

    /**
     * A turn lane tried (the header of the field lane it rides on and its sector), or a band tried
     * (the shown frame and the band, in [sector]).
     */
    internal class Tried(val header: PetalLaneHeader, val sector: Int)

    /** The turn lanes tried last, oldest first; bounded by [TURN_MEMORY]. */
    internal val tried = ArrayDeque<Tried>()

    /** Tries in a row that read no turn lane (at most [TURN_BACKOFF]). */
    internal var turnMisses = 0
        private set

    /** Captures still to pass over before the next try. */
    internal var turnWait = 0

    private val turnSkip = BooleanArray(PetalLayout.SECTORS)
    private val turnHeaders = arrayOfNulls<PetalLaneHeader>(PetalLayout.SECTORS)
    private val turnResults = arrayOfNulls<PetalLaneResult>(PetalLayout.SECTORS)

    /** The bands tried last as `(shown frame, band)`, oldest first; bounded by [KANA_MEMORY]. */
    internal val kanaTried = ArrayDeque<Tried>()

    /** Tries in a row that read no kana lane (at most [KANA_BACKOFF]). */
    internal var kanaMisses = 0
        private set

    /** Captures still to pass over before the next band try. */
    internal var kanaWait = 0

    private val kanaSkip = BooleanArray(PetalLayout.KANA_BANDS)
    private val kanaResults = arrayOfNulls<PetalKanaResult>(PetalKanaLane.ALL.size)

    /** Diagnostic counters. */
    @Synchronized
    fun stats(): PetalScanStats = PetalScanStats(
        frames, located, readable, laneD, fieldLanes, tracked, inferred, merges, mergedLanes, turnTries, turnLanes,
        kanaTries, kanaLanes, spareTurns,
    )

    /** Current progress. */
    @Synchronized
    fun progress(): PetalProgress = assembler.progress()

    /** Drops all partial state, including the pose that tracking follows. */
    @Synchronized
    fun reset() {
        assembler.reset()
        startedMillis = null
        lastRank = 0
        lastPose = null
        held.clear()
        forgetTurns()
    }

    /** Forgets the turn lanes and bands tried and any back-off. */
    private fun forgetTurns() {
        tried.clear()
        turnMisses = 0
        turnWait = 0
        kanaTried.clear()
        kanaMisses = 0
        kanaWait = 0
    }

    /** Offers one camera luma plane captured at monotonic time [nowMillis] (non-negative). */
    @Synchronized
    fun push(image: PetalLuma, nowMillis: Long): PetalScanOutcome {
        require(nowMillis >= 0) { "scan time must not be negative" }
        val started = startedMillis
        if (started != null &&
            (saturatingElapsed(nowMillis, progressMillis) > limits.idleTimeoutMillis ||
                saturatingElapsed(nowMillis, started) > limits.absoluteTimeoutMillis)
        ) {
            reset()
        }
        frames += 1
        val pushStarted = System.nanoTime()
        val previous = lastPose
        val followed = if (previous != null && saturatingElapsed(nowMillis, lastPoseMillis) <= TRACK_WINDOW_MILLIS) {
            PetalDecoder.track(image, previous, limits.decode, workspace)
        } else {
            null
        }
        if (followed != null) tracked += 1
        val result = if (followed != null) PetalDecodeResult(followed, null) else PetalDecoder.decode(image, limits.decode, workspace)
        val frame = result.frame
        var lanes = ""
        var merged = ""
        var kanaRead = ""
        var turnedLanes = ""
        if (frame != null) {
            if (frame.inferredCorner != null) inferred += 1
            lastPose = frame
            lastPoseMillis = nowMillis
            lanes = absorb(frame)
            var spent = cost(if (followed != null) PetalWork.Track else PetalWork.Search, pushStarted)
            val mergesBefore = merges
            var partStarted = System.nanoTime()
            merged = merge(image, frame)
            if (merges > mergesBefore) spent += cost(PetalWork.Merge, partStarted)
            val bandsBefore = kanaTries
            partStarted = System.nanoTime()
            kanaRead = kana(image, frame)
            val bands = (kanaTries - bandsBefore).toInt()
            if (bands > 0) spent += cost(PetalWork.Bands(bands), partStarted)
            // a capture whose bands read a lane tries the turn lanes only with time to spare (see the class docs)
            turnedLanes = when {
                kanaRead.isEmpty() -> turn(image, frame)
                spent + (turnCostMillis ?: (2 * spent)) <= CAPTURE_BUDGET_MILLIS -> {
                    val triesBefore = turnTries
                    turn(image, frame).also { if (turnTries > triesBefore) spareTurns += 1 }
                }
                else -> ""
            }
        }
        if (result.error != PetalDecodeError.NO_FINDERS && result.error != PetalDecodeError.UNSUPPORTED_IMAGE) {
            located += 1
        }
        val progress = assembler.progress()
        if (progress.rank > lastRank || (progress.meta != null && startedMillis == null)) {
            progressMillis = nowMillis
            if (startedMillis == null) startedMillis = nowMillis
        }
        lastRank = progress.rank
        return PetalScanOutcome(result.error, lanes, merged, turnedLanes, kanaRead, progress, assembler.takeCompleted())
    }

    /**
     * Tries the bands not yet tried for this capture's shown frame and offers the atoms of the kana
     * lanes they read; after tries that read nothing it passes over the next captures that would
     * try one (see the class docs).
     */
    internal fun kana(image: PetalLuma, frame: PetalDecodedFrame): String {
        val header = shownFrame(frame) ?: return ""
        var all = true
        for (band in 0 until PetalLayout.KANA_BANDS) {
            kanaSkip[band] = kanaTried.any { it.sector == band && it.header == header }
            if (!kanaSkip[band]) all = false
        }
        if (all) return ""
        if (kanaWait > 0) {
            kanaWait -= 1
            return ""
        }
        for (band in 0 until PetalLayout.KANA_BANDS) {
            if (kanaSkip[band]) continue
            if (kanaTried.size == KANA_MEMORY) kanaTried.removeFirst()
            kanaTried.addLast(Tried(header, band))
            kanaTries += 1
        }
        workspace.kanaReader.read(image, frame, header, kanaSkip, kanaResults)
        val read = StringBuilder(2 * PetalKanaLane.ALL.size)
        for (lane in PetalKanaLane.ALL) {
            val result = kanaResults[lane.ordinal] ?: continue
            val packet = PetalKana.kanaPacket(result.header, lane, result.result.dataView) ?: continue
            assembler.pushAtoms(packet)
            kanaLanes += 1
            read.append(lane.name)
        }
        if (read.isEmpty()) {
            kanaMisses = minOf(kanaMisses + 1, KANA_BACKOFF)
            kanaWait = kanaMisses
        } else {
            kanaMisses = 0
        }
        return read.toString()
    }

    internal fun absorb(frame: PetalDecodedFrame): String {
        val lanes = frame.lanes
        if (frame.d != null) laneD += 1
        fieldLanes += frame.fields.count { it != null }
        if (lanes.isNotEmpty()) readable += 1
        val previousMeta = assembler.progress().meta
        frame.feed(assembler)
        // A short tag can collide: accepted metadata replacement starts a new capture cache.
        // Keep pre-beacon evidence when the first stream identity is learned.
        if (previousMeta != null && assembler.progress().meta != previousMeta) {
            held.clear()
            forgetTurns()
        }
        return lanes
    }

    /**
     * Tries the turn lanes on top of the field lanes this capture decoded, except those already
     * tried for their shown frame, and offers the atoms of those it reads; after tries that read
     * nothing it passes over the next captures that would try one (see the class docs).
     */
    internal fun turn(image: PetalLuma, frame: PetalDecodedFrame): String {
        var all = true
        for (sector in 0 until PetalLayout.SECTORS) {
            turnSkip[sector] = true
            turnHeaders[sector] = null
            val header = frame.field(sector)?.let { laneHeader(it.dataView) } ?: continue
            turnHeaders[sector] = header
            turnSkip[sector] = tried.any { it.sector == sector && it.header == header }
            if (!turnSkip[sector]) all = false
        }
        if (all) return ""
        if (turnWait > 0) {
            turnWait -= 1
            return ""
        }
        for (sector in 0 until PetalLayout.SECTORS) {
            val header = turnHeaders[sector] ?: continue
            if (turnSkip[sector]) continue
            if (tried.size == TURN_MEMORY) tried.removeFirst()
            tried.addLast(Tried(header, sector))
            turnTries += 1
        }
        val readStarted = System.nanoTime()
        workspace.turnReader.read(image, frame, turnSkip, turnResults)
        turnCostMillis = cost(PetalWork.Turns, readStarted)
        val letters = StringBuilder(PetalLayout.SECTORS)
        for (sector in 0 until PetalLayout.SECTORS) {
            val result = turnResults[sector] ?: continue
            val header = turnHeaders[sector] ?: continue
            val packet = PetalTurns.turnPacket(header, sector, result.dataView) ?: continue
            assembler.pushAtoms(packet)
            turnLanes += 1
            letters.append(PetalLane.field(sector).letter)
        }
        if (letters.isEmpty()) {
            turnMisses = minOf(turnMisses + 1, TURN_BACKOFF)
            turnWait = turnMisses
        } else {
            turnMisses = 0
        }
        return letters.toString()
    }

    /**
     * Holds this capture's field evidence under its shown frame and, from the second capture on,
     * merges the held captures of that frame.
     */
    internal fun merge(image: PetalLuma, frame: PetalDecodedFrame): String {
        val shown = shownFrame(frame) ?: return ""
        val captures = held.firstOrNull { it.shown == shown } ?: Held(shown).also {
            if (held.size == MERGE_WINDOW) held.removeFirst()
            held.addLast(it)
        }
        for (sector in 0 until PetalLayout.SECTORS) captures.done[sector] = captures.done[sector] || frame.field(sector) != null
        if (captures.complete) {
            captures.evidence.clear()
            return ""
        }
        // a full window gives up its oldest capture, whose buffer the new evidence reuses
        val recycled = if (captures.evidence.size == MERGE_CAPTURES) captures.evidence.first.levels else null
        val evidence = PetalDecoder.fieldEvidence(image, frame, limits.decode, recycled) ?: return ""
        if (captures.evidence.size == MERGE_CAPTURES) captures.evidence.removeFirst()
        captures.evidence.addLast(evidence)
        if (captures.evidence.size < 2) return ""
        merges += 1
        val results = PetalDecoder.mergeFields(captures.evidence, captures.done, workspace)
        val merged = StringBuilder(PetalLayout.SECTORS)
        for (lane in PetalLane.FIELDS) {
            val packet = results[lane.sector]?.let { PetalStream.parseAtomLane(lane, it.dataView) } ?: continue
            // a merge offers only atoms of the shown frame it was made for
            if (packet.header != shown) continue
            captures.done[lane.sector] = true
            assembler.pushAtoms(packet)
            mergedLanes += 1
            merged.append(lane.letter)
        }
        if (captures.complete) captures.evidence.clear()
        return merged.toString()
    }

    /**
     * The shown frame every decoded lane header of [frame] names; `null` when no lane decoded or
     * the lanes of a torn capture name different frames.
     */
    private fun shownFrame(frame: PetalDecodedFrame): PetalLaneHeader? {
        var shown: PetalLaneHeader? = null
        for (lane in PetalLane.ALL) {
            val bytes = frame.lane(lane)?.dataView ?: continue
            val header = laneHeader(bytes) ?: return null
            if (shown != null && shown != header) return null
            shown = header
        }
        return shown
    }

    /** The header every lane starts with; `null` for fewer than three bytes. */
    private fun laneHeader(bytes: ByteArray): PetalLaneHeader? {
        if (bytes.size < 3) return null
        return PetalLaneHeader(bytes[0].toInt() and 0xFF, ((bytes[1].toInt() and 0xFF) shl 8) or (bytes[2].toInt() and 0xFF))
    }

    private fun saturatingElapsed(now: Long, since: Long): Long = if (now > since) now - since else 0L

    /** What [work] cost, given that it started at [startedNanos] on the monotonic clock. */
    private fun cost(work: PetalWork, startedNanos: Long): Double =
        workCost.cost(work, (System.nanoTime() - startedNanos) / 1e6)

    companion object {
        /** How long a decoded pose stays usable for tracking the next frames. */
        const val TRACK_WINDOW_MILLIS = 500L

        /** Shown frames whose captures are held for merging. */
        const val MERGE_WINDOW = 6

        /** Captures held per shown frame; a further capture replaces the oldest. */
        const val MERGE_CAPTURES = 4

        /** Turn lanes remembered as tried (the four of each of the last [MERGE_WINDOW] shown frames). */
        const val TURN_MEMORY = MERGE_WINDOW * PetalLayout.SECTORS

        /** Most captures passed over after tries that read no turn lane. */
        const val TURN_BACKOFF = 8

        /** Bands remembered as tried (the two of each of the last [MERGE_WINDOW] shown frames). */
        const val KANA_MEMORY = MERGE_WINDOW * PetalLayout.KANA_BANDS

        /** Most captures passed over after tries that read no kana lane. */
        const val KANA_BACKOFF = 8

        /**
         * The time a capture's work may take, turn lanes included, for a capture whose bands read a
         * lane to try the turn lanes too: a 30 fps camera's frame time, so a decoder that keeps to it
         * misses no camera frame.
         */
        const val CAPTURE_BUDGET_MILLIS = 1000.0 / 30.0
    }
}
