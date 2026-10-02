package org.hyperledger.iroha.sdk.offline.petal

/** Receiver limits that bound memory and work. */
class PetalAssemblerLimits @JvmOverloads constructor(
    /** Largest payload the receiver accepts. */
    val maxPayloadLength: Int = PetalStream.DEFAULT_MAX_PAYLOAD_LEN,
    /** Atoms buffered while waiting for the first beacon (`0` drops them). */
    val maxPendingAtoms: Int = 128,
) {
    init {
        require(maxPayloadLength >= 0 && maxPendingAtoms >= 0) { "assembler limits must not be negative" }
    }

    override fun equals(other: Any?): Boolean = other is PetalAssemblerLimits &&
        maxPayloadLength == other.maxPayloadLength && maxPendingAtoms == other.maxPendingAtoms

    override fun hashCode(): Int = 31 * maxPayloadLength + maxPendingAtoms

    companion object {
        /** 64 KiB payloads, 128 pending atoms. */
        @JvmField
        val DEFAULT = PetalAssemblerLimits()
    }
}

/** A reassembled, CRC-verified payload. */
class PetalCompleted internal constructor(
    /** Stream identity. */
    val meta: PetalStreamMeta,
    private val bytes: ByteArray,
) {
    /** The payload bytes (a copy). */
    val payload: ByteArray get() = bytes.copyOf()

    override fun equals(other: Any?): Boolean =
        other is PetalCompleted && meta == other.meta && bytes.contentEquals(other.bytes)

    override fun hashCode(): Int = 31 * meta.hashCode() + bytes.contentHashCode()
}

/** Snapshot of receive progress for a UI. */
class PetalProgress internal constructor(
    /** Stream identity once a beacon was accepted. */
    val meta: PetalStreamMeta?,
    /** Source atoms of the active stream. */
    val sourceAtoms: Int,
    /** Independent atoms collected so far. */
    val rank: Int,
    /** Atoms offered to the decoder, duplicates included (saturates at `2^32 - 1`). */
    val atomsReceived: Long,
    /**
     * Reassembled payloads that failed the CRC check and were discarded
     * (cumulative over the assembler's lifetime, not cleared by `reset`).
     */
    val integrityFailures: Long,
    /** Whether the payload is complete and verified. */
    val complete: Boolean,
) {
    override fun equals(other: Any?): Boolean = other is PetalProgress && meta == other.meta &&
        sourceAtoms == other.sourceAtoms && rank == other.rank && atomsReceived == other.atomsReceived &&
        integrityFailures == other.integrityFailures && complete == other.complete

    override fun hashCode(): Int = 31 * (31 * (meta?.hashCode() ?: 0) + rank) + atomsReceived.hashCode()

    override fun toString(): String =
        "PetalProgress(meta=$meta, sourceAtoms=$sourceAtoms, rank=$rank, atomsReceived=$atomsReceived, " +
            "integrityFailures=$integrityFailures, complete=$complete)"
}

/**
 * Receiver side: collects atoms from any lane of any frame.
 *
 * Atoms that arrive before the first beacon wait in a bounded buffer
 * ([PetalAssemblerLimits.maxPendingAtoms]; with `0` they are dropped). A beacon
 * of a different stream replaces the active one only after two consecutive
 * sightings. A reassembled payload is delivered only when its CRC-32C matches
 * the beacon; otherwise the elimination restarts and the cumulative
 * [PetalProgress.integrityFailures] grows. Instances are mutable and not
 * thread-safe; confine one to a single thread (or use [PetalScanSession]).
 */
class PetalStreamAssembler @JvmOverloads constructor(
    /** Memory and size limits. */
    val limits: PetalAssemblerLimits = PetalAssemblerLimits.DEFAULT,
) {
    private class Active(val meta: PetalStreamMeta) {
        var decoder = PetalFountainDecoder(meta.sourceAtoms)
        var done = false
    }

    private class Pending(val tag: Int, val id: Int, val atom: ByteArray)

    private var active: Active? = null
    private var pending = ArrayDeque<Pending>()
    private var conflicting: PetalStreamMeta? = null
    private var conflictingSeen = 0
    private var completed: PetalCompleted? = null
    private var atomsReceived = 0L
    private var integrityFailures = 0L

    /**
     * Forgets the active stream, pending atoms and any completed payload.
     * [PetalProgress.integrityFailures] stays cumulative and is not cleared.
     */
    fun reset() {
        active = null
        pending.clear()
        conflicting = null
        conflictingSeen = 0
        completed = null
        atomsReceived = 0
    }

    /** Current progress. */
    fun progress(): PetalProgress {
        val current = active
        return PetalProgress(
            current?.meta,
            current?.decoder?.sourceAtoms ?: 0,
            current?.decoder?.rank ?: 0,
            atomsReceived,
            integrityFailures,
            current?.done ?: false,
        )
    }

    /** Takes the completed payload, if any; it is delivered exactly once. */
    fun takeCompleted(): PetalCompleted? = completed.also { completed = null }

    /** Offers a beacon read from lane `D`. */
    fun pushBeacon(beacon: PetalBeacon) {
        val meta = beacon.meta
        if (meta.length == 0 || meta.length > limits.maxPayloadLength) return
        val current = active
        when {
            current == null -> start(meta)
            current.meta == meta -> conflicting = null
            else -> {
                // A different stream: switch only after two consecutive sightings.
                val seen = if (conflicting == meta) conflictingSeen + 1 else 1
                if (seen >= 2) {
                    start(meta)
                } else {
                    conflicting = meta
                    conflictingSeen = seen
                }
            }
        }
    }

    /** Offers atoms read from a lane. */
    fun pushAtoms(packet: PetalAtomPacket) {
        for (index in 0 until packet.atomCount) {
            val id = packet.firstId + index
            val current = active
            if (current != null) {
                if (current.meta.tag == packet.header.tag) addAtom(id, packet.atom(index))
            } else {
                if (limits.maxPendingAtoms == 0) continue
                if (pending.size >= limits.maxPendingAtoms) pending.removeFirstOrNull()
                pending.addLast(Pending(packet.header.tag, id, packet.atom(index)))
            }
        }
    }

    /** Offers whatever lane `D` carried. */
    fun pushDLane(lane: PetalDLane) {
        lane.beacon?.let(::pushBeacon)
        lane.atoms?.let(::pushAtoms)
    }

    private fun start(meta: PetalStreamMeta) {
        active = Active(meta)
        conflicting = null
        conflictingSeen = 0
        completed = null
        atomsReceived = 0
        val tag = meta.tag
        val waiting = pending
        pending = ArrayDeque()
        for (entry in waiting) {
            if (entry.tag == tag) addAtom(entry.id, entry.atom)
        }
    }

    private fun addAtom(id: Int, atom: ByteArray) {
        val current = active ?: return
        if (current.done) return
        if (atomsReceived < MAX_U32) atomsReceived += 1
        current.decoder.addEncoded(current.meta.crc, id, atom)
        if (!current.decoder.isComplete) return
        val source = current.decoder.solveFlat() ?: return
        val payload = source.copyOf(current.meta.length)
        if (PetalCrc32c.compute(payload) == current.meta.crc) {
            current.done = true
            completed = PetalCompleted(current.meta, payload)
        } else {
            // Corrupt atoms slipped through: start the elimination over.
            if (integrityFailures < MAX_U32) integrityFailures += 1
            current.decoder = PetalFountainDecoder(current.meta.sourceAtoms)
        }
    }

    private companion object {
        const val MAX_U32 = 0xFFFF_FFFFL
    }
}
