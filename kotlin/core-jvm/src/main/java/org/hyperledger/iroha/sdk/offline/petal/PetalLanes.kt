package org.hyperledger.iroha.sdk.offline.petal

/**
 * One of the three data lanes of a Petal frame. Each lane is exactly one
 * Reed–Solomon codeword, XOR-whitened with a fixed xorshift32 sequence.
 *
 * | lane | cells | codeword | data | parity |
 * |------|-------|----------|------|--------|
 * | `P` polarity | 256 tiles × 1 bit | 32 B | 19 B | 13 B |
 * | `K` katakana | 256 tiles × 4 bits | 128 B | 83 B | 45 B |
 * | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
 */
enum class PetalLane(
    /** The lane letter used in diagnostics (`P`, `K` or `D`). */
    val letter: Char,
    /** Codeword length in bytes. */
    val wordLength: Int,
    /** Parity bytes. */
    val parityLength: Int,
    whiteningSeed: Int,
) {
    /** Light/dark polarity of the tiles. */
    P('P', PetalLanes.P_WORD, PetalLanes.P_PARITY, 0x5045_5441), // "PETA"

    /** Katakana glyph of each tile. */
    K('K', PetalLanes.K_WORD, PetalLanes.K_PARITY, 0x4B41_4E41), // "KANA"

    /** Dots on the three rings. */
    D('D', PetalLanes.D_WORD, PetalLanes.D_PARITY, 0x444F_5453), // "DOTS"
    ;

    /** Data bytes. */
    val dataLength: Int get() = wordLength - parityLength

    internal val whiteningBytes: ByteArray = PetalXorshift32(whiteningSeed).let { rng ->
        ByteArray(wordLength) { rng.nextByte().toByte() }
    }

    internal val codec = PetalReedSolomon(parityLength)

    /** The fixed whitening sequence of the lane. */
    fun whitening(): ByteArray = whiteningBytes.copyOf()

    companion object {
        /** All lanes in decode order: `P`, `D`, `K`. */
        @JvmField
        val DECODE_ORDER: List<PetalLane> = java.util.Collections.unmodifiableList(listOf(P, D, K))
    }
}

/** Lane constants and codecs: bytes ⇄ transmitted (whitened) codewords. */
object PetalLanes {
    /** Length of a fountain atom in bytes. */
    const val ATOM_LEN = 16

    /** Bytes of the per-lane header (`tag`, `frame` high, `frame` low). */
    const val LANE_HEADER_LEN = 3

    /** Atoms carried by lane `P`. */
    const val P_ATOMS = 1

    /** Atoms carried by lane `D` on frames that do not carry a beacon. */
    const val D_ATOMS = 1

    /** Atoms carried by lane `K`. */
    const val K_ATOMS = 5

    /** Most atoms one frame can carry (lanes `P`, `D` and `K`). */
    const val ATOMS_PER_FRAME = P_ATOMS + D_ATOMS + K_ATOMS

    /** Codeword length of lane `P` in bytes. */
    const val P_WORD = PetalLayout.TILE_COUNT / 8

    /** Codeword length of lane `K` in bytes. */
    const val K_WORD = PetalLayout.TILE_COUNT / 2

    /** Codeword length of lane `D` in bytes. */
    const val D_WORD = PetalLayout.D_BITS / 8

    /** Parity bytes of lane `P`. */
    const val P_PARITY = 13

    /** Parity bytes of lane `K`. */
    const val K_PARITY = 45

    /** Parity bytes of lane `D`. */
    const val D_PARITY = 11

    /** Data bytes of lane `P`. */
    const val P_DATA = P_WORD - P_PARITY

    /** Data bytes of lane `K`. */
    const val K_DATA = K_WORD - K_PARITY

    /** Data bytes of lane `D`. */
    const val D_DATA = D_WORD - D_PARITY

    init {
        check(P_DATA == LANE_HEADER_LEN + P_ATOMS * ATOM_LEN)
        check(K_DATA == LANE_HEADER_LEN + K_ATOMS * ATOM_LEN)
        check(D_DATA == LANE_HEADER_LEN + D_ATOMS * ATOM_LEN)
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
    private val lightCells: BooleanArray,
    private val glyphCells: ByteArray,
    private val dotCells: BooleanArray,
) {
    /** Whether tile [tile] is light. */
    fun isLight(tile: Int): Boolean = lightCells[tile]

    /** Glyph symbol (`0..15`) of tile [tile]. */
    fun glyph(tile: Int): Int = glyphCells[tile].toInt()

    /** Whether flat ring slot [slot] is lit (gate dots included). */
    fun isDotLit(slot: Int): Boolean = dotCells[slot]

    /** Polarity of each tile; `true` is a light tile. */
    fun light(): BooleanArray = lightCells.copyOf()

    /** Glyph symbol (`0..15`) of each tile. */
    fun glyphs(): IntArray = IntArray(PetalLayout.TILE_COUNT) { glyphCells[it].toInt() }

    /** Lit state of every ring slot, gate dots included. */
    fun dots(): BooleanArray = dotCells.copyOf()

    /** Packs the polarity cells into a lane `P` codeword. */
    fun pWord(): ByteArray {
        val word = ByteArray(PetalLanes.P_WORD)
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            if (lightCells[tile]) word[tile / 8] = (word[tile / 8].toInt() or (1 shl (7 - tile % 8))).toByte()
        }
        return word
    }

    /** Packs the glyph cells into a lane `K` codeword (first tile of a pair in the high nibble). */
    fun kWord(): ByteArray {
        val word = ByteArray(PetalLanes.K_WORD)
        for (tile in 0 until PetalLayout.TILE_COUNT) {
            val nibble = glyphCells[tile].toInt() and 0x0F
            val shifted = if (tile % 2 == 0) nibble shl 4 else nibble
            word[tile / 2] = (word[tile / 2].toInt() or shifted).toByte()
        }
        return word
    }

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

    override fun equals(other: Any?): Boolean = other is PetalFrameCells &&
        lightCells.contentEquals(other.lightCells) && glyphCells.contentEquals(other.glyphCells) &&
        dotCells.contentEquals(other.dotCells)

    override fun hashCode(): Int =
        31 * (31 * lightCells.contentHashCode() + glyphCells.contentHashCode()) + dotCells.contentHashCode()

    companion object {
        /**
         * Builds cells from explicit values: 256 polarities, 256 glyph symbols
         * (`0..15`) and [PetalLayout.TOTAL_SLOTS] dot states.
         */
        @JvmStatic
        fun of(light: BooleanArray, glyph: IntArray, dots: BooleanArray): PetalFrameCells {
            require(light.size == PetalLayout.TILE_COUNT && glyph.size == PetalLayout.TILE_COUNT) {
                "frame cells need ${PetalLayout.TILE_COUNT} tiles"
            }
            require(dots.size == PetalLayout.TOTAL_SLOTS) { "frame cells need ${PetalLayout.TOTAL_SLOTS} dots" }
            require(glyph.all { it in 0 until PetalGlyphs.GLYPH_COUNT }) { "glyph symbols are 0..15" }
            return PetalFrameCells(light.copyOf(), ByteArray(glyph.size) { glyph[it].toByte() }, dots.copyOf())
        }

        /** Builds the cells from the three transmitted codewords. */
        @JvmStatic
        fun fromWords(p: ByteArray, k: ByteArray, d: ByteArray): PetalFrameCells {
            require(p.size == PetalLanes.P_WORD && k.size == PetalLanes.K_WORD && d.size == PetalLanes.D_WORD) {
                "lane codeword length mismatch"
            }
            val light = BooleanArray(PetalLayout.TILE_COUNT)
            val glyph = ByteArray(PetalLayout.TILE_COUNT)
            for (tile in 0 until PetalLayout.TILE_COUNT) {
                light[tile] = ((p[tile / 8].toInt() ushr (7 - tile % 8)) and 1) == 1
                val byte = k[tile / 2].toInt() and 0xFF
                glyph[tile] = (if (tile % 2 == 0) byte ushr 4 else byte and 0x0F).toByte()
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
            return PetalFrameCells(light, glyph, dots)
        }
    }
}
