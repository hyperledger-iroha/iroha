package org.hyperledger.iroha.sdk.offline.petal

/**
 * Petal payload streams: frame schedule, lane layouts and parsers.
 *
 * Every frame carries a handful of fountain atoms, one lane at a time: lane `P`
 * one atom, lane `K` five, and lane `D` one atom — except on every fourth frame
 * (`frame % 4 == 0`), when lane `D` carries the stream *beacon* instead, so a
 * receiver can join at any frame within a fraction of a second. Atom ids run
 * contiguously over the atoms actually sent (see [firstAtomId]). Any single
 * readable lane is useful on its own.
 *
 * Lane data layouts (all big-endian):
 *
 * * every lane starts with `tag:u8, frame:u16`; `tag` is the low byte of the
 *   payload CRC-32C.
 * * lanes `P`, `K` and non-beacon `D`: `atoms…` (16 bytes each).
 * * beacon `D`: `version:u8, kind:u8, len:u24, crc:u32`, zero padded.
 *
 * Frame numbers are `u16` values (`0..65535`) and wrap.
 */
object PetalStream {
    /** Version/profile byte of the beacon: format version 1, layout profile 0. */
    const val FORMAT_VERSION = 0x10

    /** Largest payload a beacon can describe (`u24`). */
    const val MAX_PAYLOAD_LEN = (1 shl 24) - 1

    /** Default receiver payload limit; override with [PetalAssemblerLimits]. */
    const val DEFAULT_MAX_PAYLOAD_LEN = 65_536

    /** A beacon replaces the lane-`D` atom on frames divisible by this interval. */
    const val BEACON_INTERVAL = 4

    /** Largest frame number; the counter wraps to `0` after it. */
    const val MAX_FRAME = 0xFFFF

    /** Whether [frame] carries the beacon in lane `D`. */
    @JvmStatic
    fun isBeaconFrame(frame: Int): Boolean = checkFrame(frame) % BEACON_INTERVAL == 0

    /** Fountain atoms carried by [frame]. */
    @JvmStatic
    fun atomsInFrame(frame: Int): Int = if (isBeaconFrame(frame)) {
        PetalLanes.P_ATOMS + PetalLanes.K_ATOMS
    } else {
        PetalLanes.ATOMS_PER_FRAME
    }

    /**
     * Fountain id of the first atom of [frame]: frame `f` follows `f` earlier
     * frames, `ceil(f / 4)` of which were beacon frames with one atom fewer.
     */
    @JvmStatic
    fun firstAtomId(frame: Int): Int {
        val f = checkFrame(frame)
        return f * PetalLanes.ATOMS_PER_FRAME - (f + BEACON_INTERVAL - 1) / BEACON_INTERVAL
    }

    /** Fountain id of the first atom [lane] carries in [frame]. */
    internal fun laneFirstId(lane: PetalLane, frame: Int): Int {
        val base = firstAtomId(frame)
        return when (lane) {
            PetalLane.P -> base
            PetalLane.D -> base + PetalLanes.P_ATOMS
            PetalLane.K -> base + PetalLanes.P_ATOMS + if (isBeaconFrame(frame)) 0 else PetalLanes.D_ATOMS
        }
    }

    /** Parses the data bytes of lane `P` or lane `K`; `null` for lane `D` or a length mismatch. */
    @JvmStatic
    fun parseAtomLane(lane: PetalLane, data: ByteArray): PetalAtomPacket? {
        val count = when (lane) {
            PetalLane.P -> PetalLanes.P_ATOMS
            PetalLane.K -> PetalLanes.K_ATOMS
            PetalLane.D -> return null
        }
        if (data.size != lane.dataLength) return null
        return parseAtoms(lane, data, parseHeader(data), count)
    }

    /** Parses the data bytes of lane `D`; `null` for a malformed beacon or length. */
    @JvmStatic
    fun parseDLane(data: ByteArray): PetalDLane? {
        if (data.size != PetalLanes.D_DATA) return null
        val header = parseHeader(data)
        if (!isBeaconFrame(header.frame)) {
            return PetalDLane(null, parseAtoms(PetalLane.D, data, header, PetalLanes.D_ATOMS))
        }
        val body = PetalLanes.LANE_HEADER_LEN
        if ((data[body].toInt() and 0xFF) != FORMAT_VERSION) return null
        val length = (u8(data, body + 2) shl 16) or (u8(data, body + 3) shl 8) or u8(data, body + 4)
        if (length == 0) return null
        val crc = (u8(data, body + 5) shl 24) or (u8(data, body + 6) shl 16) or
            (u8(data, body + 7) shl 8) or u8(data, body + 8)
        return PetalDLane(PetalBeacon(header, PetalStreamMeta(u8(data, body + 1), length, crc)), null)
    }

    private fun parseHeader(data: ByteArray): PetalLaneHeader =
        PetalLaneHeader(u8(data, 0), (u8(data, 1) shl 8) or u8(data, 2))

    private fun parseAtoms(lane: PetalLane, data: ByteArray, header: PetalLaneHeader, count: Int): PetalAtomPacket {
        val atoms = data.copyOfRange(PetalLanes.LANE_HEADER_LEN, PetalLanes.LANE_HEADER_LEN + count * PetalLanes.ATOM_LEN)
        return PetalAtomPacket(header, laneFirstId(lane, header.frame), atoms)
    }

    private fun u8(data: ByteArray, index: Int): Int = data[index].toInt() and 0xFF

    internal fun checkFrame(frame: Int): Int {
        require(frame in 0..MAX_FRAME) { "frame number must be 0..65535" }
        return frame
    }
}

/** Identity of a stream, as carried by every beacon. */
class PetalStreamMeta(
    /** Application payload kind (`0..255`). */
    val kind: Int,
    /** Payload length in bytes (at most [PetalStream.MAX_PAYLOAD_LEN]; beacons never carry `0`). */
    val length: Int,
    /** CRC-32C of the payload (raw 32-bit pattern). */
    val crc: Int,
) {
    init {
        require(kind in 0..0xFF) { "payload kind must be a byte" }
        require(length in 0..PetalStream.MAX_PAYLOAD_LEN) { "payload length must fit 24 bits" }
    }

    /** The one-byte stream tag repeated in every lane header. */
    val tag: Int get() = crc and 0xFF

    /** Number of fountain source atoms. */
    val sourceAtoms: Int get() = (length + PetalLanes.ATOM_LEN - 1) / PetalLanes.ATOM_LEN

    override fun equals(other: Any?): Boolean =
        other is PetalStreamMeta && kind == other.kind && length == other.length && crc == other.crc

    override fun hashCode(): Int = 31 * (31 * kind + length) + crc

    override fun toString(): String =
        "PetalStreamMeta(kind=$kind, length=$length, crc=0x${Integer.toHexString(crc)})"
}

/** The common three-byte header of every lane. */
class PetalLaneHeader(
    /** Stream tag (`0..255`). */
    val tag: Int,
    /** Frame counter (`0..65535`, wraps). */
    val frame: Int,
) {
    init {
        require(tag in 0..0xFF) { "lane tag must be a byte" }
        PetalStream.checkFrame(frame)
    }

    override fun equals(other: Any?): Boolean = other is PetalLaneHeader && tag == other.tag && frame == other.frame

    override fun hashCode(): Int = 31 * tag + frame

    override fun toString(): String = "PetalLaneHeader(tag=$tag, frame=$frame)"
}

/** A decoded beacon. */
class PetalBeacon(
    /** Lane header. */
    val header: PetalLaneHeader,
    /** Stream identity. */
    val meta: PetalStreamMeta,
) {
    override fun equals(other: Any?): Boolean = other is PetalBeacon && header == other.header && meta == other.meta

    override fun hashCode(): Int = 31 * header.hashCode() + meta.hashCode()

    override fun toString(): String = "PetalBeacon(header=$header, meta=$meta)"
}

/** Fountain atoms read from one lane; ids follow [firstId] consecutively. */
class PetalAtomPacket internal constructor(
    /** Lane header. */
    val header: PetalLaneHeader,
    /** Fountain id of the first atom (raw unsigned 32-bit pattern). */
    val firstId: Int,
    /** Atoms back to back; owned by the packet. */
    private val atomBytes: ByteArray,
) {
    init {
        require(atomBytes.size % PetalLanes.ATOM_LEN == 0) { "atoms are ${PetalLanes.ATOM_LEN} bytes" }
    }

    /** Creates a packet from explicit atoms of [PetalLanes.ATOM_LEN] bytes each. */
    constructor(header: PetalLaneHeader, firstId: Int, atoms: List<ByteArray>) : this(
        header,
        firstId,
        ByteArray(atoms.size * PetalLanes.ATOM_LEN).also { flat ->
            atoms.forEachIndexed { index, atom ->
                require(atom.size == PetalLanes.ATOM_LEN) { "atoms are ${PetalLanes.ATOM_LEN} bytes" }
                atom.copyInto(flat, index * PetalLanes.ATOM_LEN)
            }
        },
    )

    /** Number of atoms. */
    val atomCount: Int get() = atomBytes.size / PetalLanes.ATOM_LEN

    /** Atom [index] (a copy). */
    fun atom(index: Int): ByteArray {
        require(index in 0 until atomCount) { "atom index out of range" }
        return atomBytes.copyOfRange(index * PetalLanes.ATOM_LEN, (index + 1) * PetalLanes.ATOM_LEN)
    }

    /** All atoms (copies). */
    fun atoms(): List<ByteArray> = List(atomCount) { atom(it) }

    internal fun atomByte(index: Int, byte: Int): Byte = atomBytes[index * PetalLanes.ATOM_LEN + byte]

    override fun equals(other: Any?): Boolean = other is PetalAtomPacket && header == other.header &&
        firstId == other.firstId && atomBytes.contentEquals(other.atomBytes)

    override fun hashCode(): Int = 31 * (31 * header.hashCode() + firstId) + atomBytes.contentHashCode()
}

/** What lane `D` carried: exactly one of [beacon] and [atoms] is non-null. */
class PetalDLane internal constructor(
    /** The stream beacon, on beacon frames. */
    val beacon: PetalBeacon?,
    /** A payload atom, on other frames. */
    val atoms: PetalAtomPacket?,
) {
    init {
        require((beacon == null) != (atoms == null)) { "lane D carries either a beacon or atoms" }
    }

    /** Whether lane `D` carried the beacon. */
    val isBeacon: Boolean get() = beacon != null

    override fun equals(other: Any?): Boolean = other is PetalDLane && beacon == other.beacon && atoms == other.atoms

    override fun hashCode(): Int = 31 * (beacon?.hashCode() ?: 0) + (atoms?.hashCode() ?: 0)

    companion object {
        /** Lane `D` carrying [beacon]. */
        @JvmStatic
        fun ofBeacon(beacon: PetalBeacon): PetalDLane = PetalDLane(beacon, null)

        /** Lane `D` carrying [packet]. */
        @JvmStatic
        fun ofAtoms(packet: PetalAtomPacket): PetalDLane = PetalDLane(null, packet)
    }
}

/** The `(P, K, D)` byte strings of one frame: lane data or transmitted codewords. */
class PetalFrameLanes internal constructor(
    private val pBytes: ByteArray,
    private val kBytes: ByteArray,
    private val dBytes: ByteArray,
) {
    /** Lane `P` bytes (a copy). */
    fun p(): ByteArray = pBytes.copyOf()

    /** Lane `K` bytes (a copy). */
    fun k(): ByteArray = kBytes.copyOf()

    /** Lane `D` bytes (a copy). */
    fun d(): ByteArray = dBytes.copyOf()

    /** The bytes of [lane] (a copy). */
    fun lane(lane: PetalLane): ByteArray = when (lane) {
        PetalLane.P -> p()
        PetalLane.K -> k()
        PetalLane.D -> d()
    }

    internal fun laneView(lane: PetalLane): ByteArray = when (lane) {
        PetalLane.P -> pBytes
        PetalLane.K -> kBytes
        PetalLane.D -> dBytes
    }

    override fun equals(other: Any?): Boolean = other is PetalFrameLanes && pBytes.contentEquals(other.pBytes) &&
        kBytes.contentEquals(other.kBytes) && dBytes.contentEquals(other.dBytes)

    override fun hashCode(): Int =
        31 * (31 * pBytes.contentHashCode() + kBytes.contentHashCode()) + dBytes.contentHashCode()
}

/**
 * Sender side: turns one payload into an endless sequence of frames.
 *
 * Immutable and thread-safe; the payload is copied on construction.
 *
 * @param payload `1..`[PetalStream.MAX_PAYLOAD_LEN] bytes.
 * @param kind application payload kind (`0..255`).
 */
class PetalStreamEncoder(payload: ByteArray, kind: Int) {
    /** Stream identity. */
    val meta: PetalStreamMeta

    private val source: ByteArray
    private val k: Int

    init {
        require(payload.isNotEmpty()) { "petal stream payload is empty" }
        require(payload.size <= PetalStream.MAX_PAYLOAD_LEN) { "petal stream payload exceeds the 24-bit length field" }
        require(kind in 0..0xFF) { "payload kind must be a byte" }
        meta = PetalStreamMeta(kind, payload.size, PetalCrc32c.compute(payload))
        k = meta.sourceAtoms
        source = payload.copyOf(k * PetalLanes.ATOM_LEN)
    }

    /** Frames needed to send every source atom once (no losses, no repair). */
    val systematicFrames: Int = run {
        var frames = 0
        var atoms = 0
        while (atoms < k) {
            atoms += PetalStream.atomsInFrame(frames and PetalStream.MAX_FRAME)
            frames += 1
        }
        frames
    }

    /** The data bytes of every lane of [frame] (`0..65535`). */
    fun laneData(frame: Int): PetalFrameLanes {
        PetalStream.checkFrame(frame)
        val mask = IntArray(PetalFountain.maskLength(k))
        val p = laneBytes(PetalLane.P, frame)
        fillAtoms(p, PetalStream.laneFirstId(PetalLane.P, frame), PetalLanes.P_ATOMS, mask)
        val kLane = laneBytes(PetalLane.K, frame)
        fillAtoms(kLane, PetalStream.laneFirstId(PetalLane.K, frame), PetalLanes.K_ATOMS, mask)
        val d = laneBytes(PetalLane.D, frame)
        if (PetalStream.isBeaconFrame(frame)) {
            var at = PetalLanes.LANE_HEADER_LEN
            d[at++] = PetalStream.FORMAT_VERSION.toByte()
            d[at++] = meta.kind.toByte()
            d[at++] = (meta.length ushr 16).toByte()
            d[at++] = (meta.length ushr 8).toByte()
            d[at++] = meta.length.toByte()
            d[at++] = (meta.crc ushr 24).toByte()
            d[at++] = (meta.crc ushr 16).toByte()
            d[at++] = (meta.crc ushr 8).toByte()
            d[at] = meta.crc.toByte()
        } else {
            fillAtoms(d, PetalStream.laneFirstId(PetalLane.D, frame), PetalLanes.D_ATOMS, mask)
        }
        return PetalFrameLanes(p, kLane, d)
    }

    /** The transmitted (whitened) codewords of every lane of [frame]. */
    fun words(frame: Int): PetalFrameLanes {
        val data = laneData(frame)
        return PetalFrameLanes(
            PetalLanes.encodeLane(PetalLane.P, data.laneView(PetalLane.P)),
            PetalLanes.encodeLane(PetalLane.K, data.laneView(PetalLane.K)),
            PetalLanes.encodeLane(PetalLane.D, data.laneView(PetalLane.D)),
        )
    }

    /** Every cell of [frame], ready to render. */
    fun cells(frame: Int): PetalFrameCells {
        val words = words(frame)
        return PetalFrameCells.fromWords(
            words.laneView(PetalLane.P),
            words.laneView(PetalLane.K),
            words.laneView(PetalLane.D),
        )
    }

    private fun laneBytes(lane: PetalLane, frame: Int): ByteArray = ByteArray(lane.dataLength).also {
        it[0] = meta.tag.toByte()
        it[1] = (frame ushr 8).toByte()
        it[2] = frame.toByte()
    }

    private fun fillAtoms(lane: ByteArray, firstId: Int, count: Int, mask: IntArray) {
        for (index in 0 until count) {
            PetalFountain.encodeAtomInto(
                source,
                k,
                meta.crc,
                firstId + index,
                mask,
                lane,
                PetalLanes.LANE_HEADER_LEN + index * PetalLanes.ATOM_LEN,
            )
        }
    }
}
