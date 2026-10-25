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
) {
    override fun equals(other: Any?): Boolean = other is PetalScanStats && frames == other.frames &&
        located == other.located && readable == other.readable && laneD == other.laneD &&
        fieldLanes == other.fieldLanes && tracked == other.tracked && inferred == other.inferred &&
        merges == other.merges && mergedLanes == other.mergedLanes

    override fun hashCode(): Int =
        (31 * frames + located + 7 * readable + 11 * laneD + 13 * fieldLanes + 17 * tracked + 19 * inferred +
            23 * merges + 29 * mergedLanes).hashCode()

    override fun toString(): String =
        "PetalScanStats(frames=$frames, located=$located, readable=$readable, laneD=$laneD, fieldLanes=$fieldLanes, " +
            "tracked=$tracked, inferred=$inferred, merges=$merges, mergedLanes=$mergedLanes)"
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
 * Thread-safe: calls are serialised, so a camera analyzer thread may push while a UI thread reads.
 */
class PetalScanSession @JvmOverloads constructor(
    /** Session limits. */
    val limits: PetalScanLimits = PetalScanLimits.DEFAULT,
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

    /** Captures of one shown frame; bounded by [MERGE_CAPTURES]. */
    internal class Held(val shown: PetalLaneHeader) {
        val evidence = ArrayDeque<PetalDecoder.FieldEvidence>()

        /** The field lanes of this shown frame read so far, by sector. */
        val done = BooleanArray(PetalLayout.SECTORS)

        val complete: Boolean get() = done.all { it }
    }

    internal val held = ArrayDeque<Held>()

    /** Diagnostic counters. */
    @Synchronized
    fun stats(): PetalScanStats = PetalScanStats(
        frames, located, readable, laneD, fieldLanes, tracked, inferred, merges, mergedLanes,
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
        val previous = lastPose
        val followed = if (previous != null && saturatingElapsed(nowMillis, lastPoseMillis) <= TRACK_WINDOW_MILLIS) {
            PetalDecoder.track(image, previous, limits.decode, workspace)
        } else {
            null
        }
        if (followed != null) tracked += 1
        val result = if (followed != null) PetalDecodeResult(followed, null) else PetalDecoder.decode(image, limits.decode, workspace)
        val frame = result.frame
        if (frame != null) {
            if (frame.inferredCorner != null) inferred += 1
            lastPose = frame
            lastPoseMillis = nowMillis
        }
        val lanes = if (frame != null) absorb(frame) else ""
        val merged = if (frame != null) merge(image, frame) else ""
        if (result.error != PetalDecodeError.NO_FINDERS && result.error != PetalDecodeError.UNSUPPORTED_IMAGE) {
            located += 1
        }
        val progress = assembler.progress()
        if (progress.rank > lastRank || (progress.meta != null && startedMillis == null)) {
            progressMillis = nowMillis
            if (startedMillis == null) startedMillis = nowMillis
        }
        lastRank = progress.rank
        return PetalScanOutcome(result.error, lanes, merged, progress, assembler.takeCompleted())
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
        if (previousMeta != null && assembler.progress().meta != previousMeta) held.clear()
        return lanes
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
            if (bytes.size < 3) return null
            val header = PetalLaneHeader(
                bytes[0].toInt() and 0xFF,
                ((bytes[1].toInt() and 0xFF) shl 8) or (bytes[2].toInt() and 0xFF),
            )
            if (shown != null && shown != header) return null
            shown = header
        }
        return shown
    }

    private fun saturatingElapsed(now: Long, since: Long): Long = if (now > since) now - since else 0L

    companion object {
        /** How long a decoded pose stays usable for tracking the next frames. */
        const val TRACK_WINDOW_MILLIS = 500L

        /** Shown frames whose captures are held for merging. */
        const val MERGE_WINDOW = 6

        /** Captures held per shown frame; a further capture replaces the oldest. */
        const val MERGE_CAPTURES = 4
    }
}
