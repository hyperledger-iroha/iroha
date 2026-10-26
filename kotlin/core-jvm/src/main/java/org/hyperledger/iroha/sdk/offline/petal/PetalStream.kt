package org.hyperledger.iroha.sdk.offline.petal

/**
 * Petal payload streams: frame schedule, lane layouts and parsers.
 *
 * Every frame carries a handful of fountain atoms, one lane at a time: each of
 * the four field lanes five atoms, and lane `D` one atom — except on every
 * fourth frame (`frame % 4 == 0`), when lane `D` carries the stream *beacon*
 * instead, so a receiver can join at any frame within a fraction of a second.
 * Atom ids run contiguously over the atoms actually sent (see [firstAtomId]).
 * Any single readable lane is useful on its own. On top of every field lane,
 * the turns of its lit petals carry four more atoms from the repair range (see
 * [PetalTurns]), so a sharp camera reads up to 37 atoms (592 bytes) from the
 * square. The katakana bands above and below the square add eight atoms each
 * (see [PetalKana]): up to 53 atoms (848 bytes) a frame.
 *
 * Lane data layouts (all big-endian):
 *
 * * every lane but the turn and kana lanes starts with `tag:u8, frame:u16`;
 *   `tag` is the low byte of the payload CRC-32C.
 * * field lanes and non-beacon `D`: `atoms…` (16 bytes each).
 * * beacon `D`: `version:u8, kind:u8, len:u24, crc:u32`, zero padded.
 * * turn lanes: `atoms… check:u32`, where the check binds the atoms to the
 *   header of the field lane they ride on.
 * * kana lanes: `atoms… check:u32`, where the check binds the atoms to the
 *   stream tag, the frame and the lane.
 *
 * Frame numbers are `u16` values (`0..65535`) and wrap.
 */
object PetalStream {
    /**
     * Version/profile byte of the beacon: format version 5, layout profile 0. Beacons of the
     * retired formats 3 (`0x30`, no katakana bands), 2 (`0x20`, unturned petals) and 1 (`0x10`,
     * katakana tiles only) are refused.
     */
    const val FORMAT_VERSION = 0x50

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
        PetalLanes.ATOMS_PER_FRAME - PetalLanes.D_ATOMS
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

    /**
     * Fountain id of the first atom [lane] carries in [frame]: the field lanes come first, in
     * sector order, then lane `D`.
     */
    internal fun laneFirstId(lane: PetalLane, frame: Int): Int {
        val base = firstAtomId(frame)
        return if (lane.isField) {
            base + lane.sector * PetalLanes.FIELD_ATOMS
        } else {
            base + PetalLayout.SECTORS * PetalLanes.FIELD_ATOMS
        }
    }

    /** Parses the data bytes of a field lane; `null` for lane `D` or a length mismatch. */
    @JvmStatic
    fun parseAtomLane(lane: PetalLane, data: ByteArray): PetalAtomPacket? {
        if (!lane.isField || data.size != lane.dataLength) return null
        return parseAtoms(lane, data, parseHeader(data), PetalLanes.FIELD_ATOMS)
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

/**
 * The bytes of every lane of one frame (lane `D`, the four field lanes, the four turn lanes and
 * the four kana lanes): lane data or transmitted codewords.
 */
class PetalFrameLanes internal constructor(
    private val dBytes: ByteArray,
    private val fieldBytes: Array<ByteArray>,
    private val turnBytes: Array<ByteArray>,
    private val kanaBytes: Array<ByteArray>,
) {
    init {
        require(fieldBytes.size == PetalLayout.SECTORS) { "a frame has ${PetalLayout.SECTORS} field lanes" }
        require(turnBytes.size == PetalLayout.SECTORS) { "a frame has ${PetalLayout.SECTORS} turn lanes" }
        require(kanaBytes.size == PetalKanaLane.ALL.size) { "a frame has ${PetalKanaLane.ALL.size} kana lanes" }
    }

    /** The bytes of kana lane [lane] (a copy). */
    fun kana(lane: PetalKanaLane): ByteArray = kanaBytes[lane.ordinal].copyOf()

    /** The kana lanes in [PetalKanaLane.ALL] order: `P0`, `K0`, `P1`, `K1` (copies). */
    fun kanas(): List<ByteArray> = java.util.Collections.unmodifiableList(kanaBytes.map { it.copyOf() })

    internal fun kanaView(lane: PetalKanaLane): ByteArray = kanaBytes[lane.ordinal]

    internal val kanaViews: List<ByteArray> get() = kanaBytes.asList()

    /** Lane `D` bytes (a copy). */
    fun d(): ByteArray = dBytes.copyOf()

    /** The bytes of the field lane of sector [sector] (`0..3`, a copy). */
    fun field(sector: Int): ByteArray {
        require(sector in 0 until PetalLayout.SECTORS) { "sector index out of range" }
        return fieldBytes[sector].copyOf()
    }

    /** The field lanes, sector 0 first (copies). */
    fun fields(): List<ByteArray> = java.util.Collections.unmodifiableList(fieldBytes.map { it.copyOf() })

    /** The bytes of the turn lane on top of the field lane of sector [sector] (`0..3`, a copy). */
    fun turn(sector: Int): ByteArray {
        require(sector in 0 until PetalLayout.SECTORS) { "sector index out of range" }
        return turnBytes[sector].copyOf()
    }

    /** The turn lanes, sector 0 first (copies). */
    fun turns(): List<ByteArray> = java.util.Collections.unmodifiableList(turnBytes.map { it.copyOf() })

    /** The bytes of [lane] (a copy). */
    fun lane(lane: PetalLane): ByteArray = laneView(lane).copyOf()

    internal fun laneView(lane: PetalLane): ByteArray = if (lane.isField) fieldBytes[lane.sector] else dBytes

    internal fun turnView(sector: Int): ByteArray = turnBytes[sector]

    override fun equals(other: Any?): Boolean = other is PetalFrameLanes && dBytes.contentEquals(other.dBytes) &&
        fieldBytes.indices.all { fieldBytes[it].contentEquals(other.fieldBytes[it]) } &&
        turnBytes.indices.all { turnBytes[it].contentEquals(other.turnBytes[it]) } &&
        kanaBytes.indices.all { kanaBytes[it].contentEquals(other.kanaBytes[it]) }

    override fun hashCode(): Int {
        var hash = dBytes.contentHashCode()
        for (field in fieldBytes) hash = 31 * hash + field.contentHashCode()
        for (turn in turnBytes) hash = 31 * hash + turn.contentHashCode()
        for (kana in kanaBytes) hash = 31 * hash + kana.contentHashCode()
        return hash
    }
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

    /** The data bytes of every lane of [frame] (`0..65535`), turn and kana lanes included. */
    fun laneData(frame: Int): PetalFrameLanes {
        PetalStream.checkFrame(frame)
        val mask = IntArray(PetalFountain.maskLength(k))
        val fields = Array(PetalLayout.SECTORS) { sector ->
            val lane = PetalLane.field(sector)
            laneBytes(lane, frame).also { fillAtoms(it, PetalStream.laneFirstId(lane, frame), PetalLanes.FIELD_ATOMS, mask) }
        }
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
        val header = PetalLaneHeader(meta.tag, frame)
        val turns = Array(PetalLayout.SECTORS) { sector ->
            val atoms = ByteArray(PetalTurns.TURN_ATOMS * PetalLanes.ATOM_LEN)
            fillAtoms(atoms, 0, PetalTurns.turnFirstId(frame, sector), PetalTurns.TURN_ATOMS, mask)
            PetalTurns.turnData(header, sector, atoms)
        }
        val kana = Array(PetalKanaLane.ALL.size) { index ->
            val lane = PetalKanaLane.ALL[index]
            val atoms = ByteArray(lane.atoms * PetalLanes.ATOM_LEN)
            fillAtoms(atoms, 0, PetalKana.kanaFirstId(frame, lane), lane.atoms, mask)
            PetalKana.kanaData(header, lane, atoms)
        }
        return PetalFrameLanes(d, fields, turns, kana)
    }

    /** The transmitted (whitened) codewords of every lane of [frame], turn and kana lanes included. */
    fun words(frame: Int): PetalFrameLanes {
        val data = laneData(frame)
        return PetalFrameLanes(
            PetalLanes.encodeLane(PetalLane.D, data.laneView(PetalLane.D)),
            Array(PetalLayout.SECTORS) { sector ->
                val lane = PetalLane.field(sector)
                PetalLanes.encodeLane(lane, data.laneView(lane))
            },
            Array(PetalLayout.SECTORS) { sector -> PetalTurns.encodeTurnLane(sector, data.turnView(sector)) },
            Array(PetalKanaLane.ALL.size) { index ->
                val lane = PetalKanaLane.ALL[index]
                PetalKana.encodeKanaLane(lane, data.kanaView(lane))
            },
        )
    }

    /** Every cell of [frame], lit, turned and with its katakana tiles, ready to render. */
    fun cells(frame: Int): PetalFrameCells {
        val words = words(frame)
        val cells = PetalFrameCells.fromWords(
            words.laneView(PetalLane.D),
            List(PetalLayout.SECTORS) { words.laneView(PetalLane.field(it)) },
        )
        PetalFrameCells.setTurns(cells.petalView, cells.turnView, List(PetalLayout.SECTORS) { words.turnView(it) })
        PetalFrameCells.setKana(cells.kanaGlyphView, cells.kanaLightView, words.kanaViews)
        return cells
    }

    private fun laneBytes(lane: PetalLane, frame: Int): ByteArray = ByteArray(lane.dataLength).also {
        it[0] = meta.tag.toByte()
        it[1] = (frame ushr 8).toByte()
        it[2] = frame.toByte()
    }

    private fun fillAtoms(lane: ByteArray, firstId: Int, count: Int, mask: IntArray) =
        fillAtoms(lane, PetalLanes.LANE_HEADER_LEN, firstId, count, mask)

    /** Writes atoms [firstId] … `firstId + count − 1` back to back into [out] from [offset]. */
    private fun fillAtoms(out: ByteArray, offset: Int, firstId: Int, count: Int, mask: IntArray) {
        for (index in 0 until count) {
            PetalFountain.encodeAtomInto(source, k, meta.crc, firstId + index, mask, out, offset + index * PetalLanes.ATOM_LEN)
        }
    }
}
