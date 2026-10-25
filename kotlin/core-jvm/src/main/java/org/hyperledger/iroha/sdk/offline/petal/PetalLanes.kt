package org.hyperledger.iroha.sdk.offline.petal

/**
 * One of the five data lanes of a Petal frame: lane `D` (the dots on the three
 * rings) or the field lane of one petal sector ([F0]–[F3]). Each lane is exactly
 * one Reed–Solomon codeword, XOR-whitened with a fixed xorshift32 sequence.
 *
 * | lane | cells | codeword | data | parity |
 * |------|-------|----------|------|--------|
 * | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
 * | `F0`–`F3` petals | 840 field cells of one sector × 1 bit | 105 B | 83 B | 22 B |
 */
enum class PetalLane(
    /** The letter that names the lane in reports: `D`, or the sector digit `0`–`3`. */
    val letter: Char,
    /** Field sector (`0..3`) of a field lane; `-1` for lane `D`. */
    val sector: Int,
    /** Codeword length in bytes. */
    val wordLength: Int,
    /** Parity bytes. */
    val parityLength: Int,
    /** Atoms carried (lane `D`: on frames without a beacon). */
    val atoms: Int,
    whiteningSeed: Int,
) {
    /** Dots on the three rings. */
    D('D', -1, PetalLanes.D_WORD, PetalLanes.D_PARITY, PetalLanes.D_ATOMS, 0x444F_5453), // "DOTS"

    /** The petals of field sector 0 (3 o'clock to 6 o'clock). */
    F0('0', 0, PetalLanes.FIELD_WORD, PetalLanes.FIELD_PARITY, PetalLanes.FIELD_ATOMS, 0x5045_5430), // "PET0"

    /** The petals of field sector 1 (6 o'clock to 9 o'clock). */
    F1('1', 1, PetalLanes.FIELD_WORD, PetalLanes.FIELD_PARITY, PetalLanes.FIELD_ATOMS, 0x5045_5431), // "PET1"

    /** The petals of field sector 2 (9 o'clock to 12 o'clock). */
    F2('2', 2, PetalLanes.FIELD_WORD, PetalLanes.FIELD_PARITY, PetalLanes.FIELD_ATOMS, 0x5045_5432), // "PET2"

    /** The petals of field sector 3 (12 o'clock to 3 o'clock). */
    F3('3', 3, PetalLanes.FIELD_WORD, PetalLanes.FIELD_PARITY, PetalLanes.FIELD_ATOMS, 0x5045_5433), // "PET3"
    ;

    /** Data bytes. */
    val dataLength: Int get() = wordLength - parityLength

    /** Whether this is a field lane ([F0]–[F3]). */
    val isField: Boolean get() = sector >= 0

    internal val whiteningBytes: ByteArray = PetalXorshift32(whiteningSeed).let { rng ->
        ByteArray(wordLength) { rng.nextByte().toByte() }
    }

    internal val codec = PetalReedSolomon(parityLength)

    /** The fixed whitening sequence of the lane. */
    fun whitening(): ByteArray = whiteningBytes.copyOf()

    companion object {
        /** All lanes in decode order: `D`, then the field lanes in sector order. */
        @JvmField
        val ALL: List<PetalLane> = java.util.Collections.unmodifiableList(listOf(D, F0, F1, F2, F3))

        /** The four field lanes in sector order. */
        @JvmField
        val FIELDS: List<PetalLane> = java.util.Collections.unmodifiableList(listOf(F0, F1, F2, F3))

        /** The field lane of sector [sector] (`0..3`). */
        @JvmStatic
        fun field(sector: Int): PetalLane = when (sector) {
            0 -> F0
            1 -> F1
            2 -> F2
            3 -> F3
            else -> throw IllegalArgumentException("field sector must be 0..3")
        }
    }
}

/** Lane constants and codecs: bytes ⇄ transmitted (whitened) codewords. */
object PetalLanes {
    /** Length of a fountain atom in bytes. */
    const val ATOM_LEN = 16

    /** Bytes of the per-lane header (`tag`, `frame` high, `frame` low). */
    const val LANE_HEADER_LEN = 3

    /** Atoms carried by lane `D` on frames that do not carry a beacon. */
    const val D_ATOMS = 1

    /** Atoms carried by each field lane. */
    const val FIELD_ATOMS = 5

    /** Most atoms one frame can carry (the four field lanes and lane `D`). */
    const val ATOMS_PER_FRAME = PetalLayout.SECTORS * FIELD_ATOMS + D_ATOMS

    /** Codeword length of lane `D` in bytes. */
    const val D_WORD = PetalLayout.D_BITS / 8

    /** Parity bytes of lane `D`. */
    const val D_PARITY = 11

    /** Data bytes of lane `D`. */
    const val D_DATA = D_WORD - D_PARITY

    /** Codeword length of a field lane in bytes. */
    const val FIELD_WORD = 105

    /** Parity bytes of a field lane. */
    const val FIELD_PARITY = 22

    /** Data bytes of a field lane. */
    const val FIELD_DATA = FIELD_WORD - FIELD_PARITY

    /** Field cells of a sector that carry no bit (they stay dark). */
    const val SPARE_SECTOR_CELLS = PetalLayout.SECTOR_CELLS - FIELD_WORD * 8

    /** Cells of a sector that carry its codeword's bits. */
    internal const val FIELD_BITS = FIELD_WORD * 8

    init {
        check(D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN)
        check(FIELD_DATA == LANE_HEADER_LEN + FIELD_ATOMS * ATOM_LEN)
        check(FIELD_BITS <= PetalLayout.SECTOR_CELLS)
    }

    /** Encodes lane [data] (exactly [PetalLane.dataLength] bytes) into the transmitted codeword. */
    @JvmStatic
    fun encodeLane(lane: PetalLane, data: ByteArray): ByteArray {
        require(data.size == lane.dataLength) { "lane data length mismatch" }
        val word = lane.codec.encode(data)
        val whitening = lane.whiteningBytes
        for (index in word.indices) word[index] = (word[index].toInt() xor whitening[index].toInt()).toByte()
        return word
    }

    /**
     * Decodes a transmitted codeword, returning the lane data bytes.
     *
     * [erasures] lists byte positions the caller distrusts.
     *
     * @throws PetalReedSolomonException when the word is malformed or uncorrectable.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeLane(lane: PetalLane, transmitted: ByteArray, erasures: IntArray = IntArray(0)): ByteArray =
        decodeLaneCounted(lane, transmitted, erasures).dataView

    /**
     * Like [decodeLane], also reporting how many byte positions the Reed–Solomon
     * decoder rewrote (erased bytes plus unflagged errors) as
     * [PetalLaneResult.corrected]; [PetalLaneResult.erasures] is the number of
     * [erasures] passed.
     *
     * @throws PetalReedSolomonException when the word is malformed or uncorrectable.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeLaneCounted(lane: PetalLane, transmitted: ByteArray, erasures: IntArray = IntArray(0)): PetalLaneResult =
        decodeLaneCountedOrNull(lane, transmitted, erasures, erasures.size)
            ?: throw PetalReedSolomonException(
                if (transmitted.size != lane.wordLength || !validErasures(transmitted.size, lane, erasures)) {
                    PetalReedSolomonException.Reason.INVALID_SHAPE
                } else {
                    PetalReedSolomonException.Reason.UNCORRECTABLE
                },
            )

    /** [decodeLaneCounted] without exceptions; uses the first [erasureCount] erasures. */
    internal fun decodeLaneCountedOrNull(
        lane: PetalLane,
        transmitted: ByteArray,
        erasures: IntArray,
        erasureCount: Int,
    ): PetalLaneResult? {
        if (transmitted.size != lane.wordLength) return null
        val whitening = lane.whiteningBytes
        val word = ByteArray(transmitted.size) { (transmitted[it].toInt() xor whitening[it].toInt()).toByte() }
        val corrected = lane.codec.decodeInPlace(word, erasures, erasureCount)
        if (corrected < 0) return null
        return PetalLaneResult(word.copyOf(lane.dataLength), corrected, erasureCount)
    }

    private fun validErasures(length: Int, lane: PetalLane, erasures: IntArray): Boolean {
        if (erasures.size > lane.parityLength) return false
        val seen = BooleanArray(length)
        for (position in erasures) {
            if (position < 0 || position >= length || seen[position]) return false
            seen[position] = true
        }
        return true
    }
}

/**
 * Every cell of one frame: what a renderer draws and a decoder samples.
 *
 * Immutable: arrays are copied on construction and access.
 */
class PetalFrameCells internal constructor(
    private val dotCells: BooleanArray,
    private val petalCells: BooleanArray,
) {
    /** Whether flat ring slot [slot] is lit (gate dots included). */
    fun isDotLit(slot: Int): Boolean = dotCells[slot]

    /** Whether field cell [cell] carries a lit petal (spare cells are dark). */
    fun isPetalLit(cell: Int): Boolean = petalCells[cell]

    /** Lit state of every ring slot, gate dots included. */
    fun dots(): BooleanArray = dotCells.copyOf()

    /** Lit state of every field cell. */
    fun petals(): BooleanArray = petalCells.copyOf()

    internal val petalView: BooleanArray get() = petalCells

    /** Packs the data dots into a lane `D` codeword. */
    fun dWord(): ByteArray {
        val word = ByteArray(PetalLanes.D_WORD)
        for (bit in 0 until PetalLayout.D_BITS) {
            if (dotCells[PetalLayout.DATA_SLOTS[bit]]) {
                word[bit / 8] = (word[bit / 8].toInt() or (1 shl (7 - bit % 8))).toByte()
            }
        }
        return word
    }

    /** Packs the petals of sector [sector] (`0..3`) into its field codeword. */
    fun fieldWord(sector: Int): ByteArray {
        require(sector in 0 until PetalLayout.SECTORS) { "sector index out of range" }
        val word = ByteArray(PetalLanes.FIELD_WORD)
        val base = sector * PetalLayout.SECTOR_CELLS
        for (bit in 0 until PetalLanes.FIELD_BITS) {
            if (petalCells[PetalLayout.SECTOR_ORDER[base + bit]]) {
                word[bit / 8] = (word[bit / 8].toInt() or (1 shl (7 - bit % 8))).toByte()
            }
        }
        return word
    }

    override fun equals(other: Any?): Boolean = other is PetalFrameCells &&
        dotCells.contentEquals(other.dotCells) && petalCells.contentEquals(other.petalCells)

    override fun hashCode(): Int = 31 * dotCells.contentHashCode() + petalCells.contentHashCode()

    companion object {
        /** Builds cells from explicit values: [PetalLayout.TOTAL_SLOTS] dot states and [PetalLayout.FIELD_CELLS] petal states. */
        @JvmStatic
        fun of(dots: BooleanArray, petals: BooleanArray): PetalFrameCells {
            require(dots.size == PetalLayout.TOTAL_SLOTS) { "frame cells need ${PetalLayout.TOTAL_SLOTS} dots" }
            require(petals.size == PetalLayout.FIELD_CELLS) { "frame cells need ${PetalLayout.FIELD_CELLS} petals" }
            return PetalFrameCells(dots.copyOf(), petals.copyOf())
        }

        /** Builds the cells from the transmitted codewords of lane `D` and the four field lanes (sector 0 first). */
        @JvmStatic
        fun fromWords(d: ByteArray, fields: List<ByteArray>): PetalFrameCells {
            require(d.size == PetalLanes.D_WORD) { "lane D codeword length mismatch" }
            require(fields.size == PetalLayout.SECTORS && fields.all { it.size == PetalLanes.FIELD_WORD }) {
                "field lane codeword length mismatch"
            }
            val dots = BooleanArray(PetalLayout.TOTAL_SLOTS)
            for (slot in 0 until PetalLayout.TOTAL_SLOTS) {
                dots[slot] = when (PetalLayout.SLOT_ROLES[slot]) {
                    PetalSlotRole.GATE -> true
                    PetalSlotRole.DATA -> {
                        val bit = PetalLayout.SLOT_BITS[slot]
                        ((d[bit / 8].toInt() ushr (7 - bit % 8)) and 1) == 1
                    }
                    else -> false
                }
            }
            val petals = BooleanArray(PetalLayout.FIELD_CELLS)
            for (sector in 0 until PetalLayout.SECTORS) {
                val word = fields[sector]
                val base = sector * PetalLayout.SECTOR_CELLS
                for (bit in 0 until PetalLanes.FIELD_BITS) {
                    petals[PetalLayout.SECTOR_ORDER[base + bit]] = ((word[bit / 8].toInt() ushr (7 - bit % 8)) and 1) == 1
                }
            }
            return PetalFrameCells(dots, petals)
        }
    }
}
