package org.hyperledger.iroha.sdk.offline.petal

/**
 * One of the five data lanes of a Petal frame: lane `D` (the dots on the three
 * rings) or the field lane of one petal sector ([F0]–[F3]). Each lane is exactly
 * one Reed–Solomon codeword, XOR-whitened with a fixed xorshift32 sequence. On
 * top of every field lane, the turns of its lit petals carry a turn lane
 * (`T0`–`T3`, see [PetalTurns]), named by the digit of its field lane.
 *
 * | lane | cells | codeword | data | parity |
 * |------|-------|----------|------|--------|
 * | `D` dots | 240 ring slots × 1 bit | 30 B | 19 B | 11 B |
 * | `F0`–`F3` petals | 840 field cells of one sector × 1 bit | 105 B | 83 B | 22 B |
 * | `T0`–`T3` turns | 360 lit petals of one sector × 2 bits | 90 B | 68 B | 22 B |
 *
 * The katakana tiles of the bands above and below the square carry four more lanes (`P0`, `K0`,
 * `P1`, `K1`, see [PetalKana]).
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
 * Every cell of one frame: what a renderer draws and a decoder samples: which ring slots and
 * petals are lit, how every petal is turned (see [PetalTurns]), and what every katakana tile of
 * the card's bands shows (see [PetalKana]).
 *
 * Immutable: arrays are copied on construction and access.
 */
class PetalFrameCells internal constructor(
    private val dotCells: BooleanArray,
    private val petalCells: BooleanArray,
    private val turnCells: ByteArray,
    private val kanaGlyphCells: ByteArray,
    private val kanaLightCells: BooleanArray,
) {
    /** The glyph (`0..15`, see [PetalGlyphs]) katakana tile [tile] shows ([PetalLayout.kanaTiles] order). */
    fun kanaGlyph(tile: Int): Int = kanaGlyphCells[tile].toInt()

    /** Whether katakana tile [tile] is light (filled) rather than dark. */
    fun isKanaLight(tile: Int): Boolean = kanaLightCells[tile]

    /** The glyph of every katakana tile. */
    fun kanaGlyphs(): ByteArray = kanaGlyphCells.copyOf()

    /** Whether every katakana tile is light. */
    fun kanaLight(): BooleanArray = kanaLightCells.copyOf()

    internal val kanaGlyphView: ByteArray get() = kanaGlyphCells

    internal val kanaLightView: BooleanArray get() = kanaLightCells

    /**
     * These cells with the data tiles of both bands showing the transmitted kana codewords
     * [words] (four, in [PetalKanaLane.ALL] order: `P0`, `K0`, `P1`, `K1`); the pilots keep their
     * fixed symbols.
     */
    fun withKana(words: List<ByteArray>): PetalFrameCells {
        val glyphs = kanaGlyphCells.copyOf()
        val light = kanaLightCells.copyOf()
        setKana(glyphs, light, words)
        return PetalFrameCells(dotCells, petalCells, turnCells, glyphs, light)
    }

    /** Packs the polarity codeword the data tiles of band [band] (`0..1`) show. */
    fun kanaPolarityWord(band: Int): ByteArray {
        require(band in 0 until PetalLayout.KANA_BANDS) { "band index out of range" }
        val word = ByteArray(PetalKana.KANA_POLARITY_WORD)
        val base = band * PetalKana.KANA_DATA_TILES
        for (i in 0 until PetalKana.KANA_POLARITY_WORD * 8) {
            if (kanaLightCells[PetalLayout.KANA_DATA_TILES[base + i]]) {
                word[i / 8] = (word[i / 8].toInt() or (1 shl (7 - i % 8))).toByte()
            }
        }
        return word
    }

    /** Packs the glyph codeword the data tiles of band [band] (`0..1`) show. */
    fun kanaGlyphWord(band: Int): ByteArray {
        require(band in 0 until PetalLayout.KANA_BANDS) { "band index out of range" }
        val word = ByteArray(PetalKana.KANA_GLYPH_WORD)
        val base = band * PetalKana.KANA_DATA_TILES
        for (i in 0 until PetalKana.KANA_DATA_TILES) {
            val nibble = kanaGlyphCells[PetalLayout.KANA_DATA_TILES[base + i]].toInt() and 0x0F
            word[i / 2] = (word[i / 2].toInt() or (if (i % 2 == 0) nibble shl 4 else nibble)).toByte()
        }
        return word
    }

    /** Whether flat ring slot [slot] is lit (gate dots included). */
    fun isDotLit(slot: Int): Boolean = dotCells[slot]

    /** Whether field cell [cell] carries a lit petal (spare cells are dark). */
    fun isPetalLit(cell: Int): Boolean = petalCells[cell]

    /**
     * Turn position (`0..3`, see [PetalTurns.turnAngle]) of field cell [cell]: zero for dark
     * petals and for lit petals that carry no turn bits.
     */
    fun turn(cell: Int): Int = turnCells[cell].toInt()

    /** Lit state of every ring slot, gate dots included. */
    fun dots(): BooleanArray = dotCells.copyOf()

    /** Lit state of every field cell. */
    fun petals(): BooleanArray = petalCells.copyOf()

    /** Turn position of every field cell. */
    fun turns(): ByteArray = turnCells.copyOf()

    internal val petalView: BooleanArray get() = petalCells

    internal val turnView: ByteArray get() = turnCells

    /**
     * These cells with the carriers of every sector turned to show its transmitted turn codeword
     * ([words], [PetalTurns.TURN_WORD] bytes each, sector 0 first; see [PetalTurns]): the
     * reference's `set_turns`. A sector with too few lit petals stays unturned.
     */
    fun withTurns(words: List<ByteArray>): PetalFrameCells {
        require(words.size == PetalLayout.SECTORS && words.all { it.size == PetalTurns.TURN_WORD }) {
            "turn lane codeword length mismatch"
        }
        val turns = turnCells.copyOf()
        setTurns(petalCells, turns, words)
        return PetalFrameCells(dotCells, petalCells, turns, kanaGlyphCells, kanaLightCells)
    }

    /**
     * Packs the turns of the carriers of sector [sector] (`0..3`) into its turn codeword, or
     * `null` when the sector has too few lit petals to carry one.
     */
    fun turnWord(sector: Int): ByteArray? {
        val chosen = PetalTurns.carriers(petalCells, sector) ?: return null
        val word = ByteArray(PetalTurns.TURN_WORD)
        for ((index, cell) in chosen.withIndex()) {
            val label = PetalTurns.turnLabel(turnCells[cell].toInt())
            word[index / 4] = (word[index / 4].toInt() or (label shl (6 - 2 * (index % 4)))).toByte()
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
        dotCells.contentEquals(other.dotCells) && petalCells.contentEquals(other.petalCells) &&
        turnCells.contentEquals(other.turnCells) && kanaGlyphCells.contentEquals(other.kanaGlyphCells) &&
        kanaLightCells.contentEquals(other.kanaLightCells)

    override fun hashCode(): Int {
        var hash = 31 * (31 * dotCells.contentHashCode() + petalCells.contentHashCode()) + turnCells.contentHashCode()
        hash = 31 * hash + kanaGlyphCells.contentHashCode()
        return 31 * hash + kanaLightCells.contentHashCode()
    }

    companion object {
        /**
         * Builds cells from explicit values: [PetalLayout.TOTAL_SLOTS] dot states,
         * [PetalLayout.FIELD_CELLS] petal states and, optionally, as many turn positions (`0..3`;
         * every petal unturned when omitted) and the glyph (`0..15`) and polarity of every one of
         * the [PetalLayout.KANA_TILE_COUNT] katakana tiles (when omitted, the pilots show their
         * fixed symbols and every data tile is light with glyph 0).
         */
        @JvmStatic
        @JvmOverloads
        fun of(
            dots: BooleanArray,
            petals: BooleanArray,
            turns: ByteArray? = null,
            kanaGlyphs: ByteArray? = null,
            kanaLight: BooleanArray? = null,
        ): PetalFrameCells {
            require(dots.size == PetalLayout.TOTAL_SLOTS) { "frame cells need ${PetalLayout.TOTAL_SLOTS} dots" }
            require(petals.size == PetalLayout.FIELD_CELLS) { "frame cells need ${PetalLayout.FIELD_CELLS} petals" }
            require(turns == null || (turns.size == PetalLayout.FIELD_CELLS && turns.all { it in 0 until PetalTurns.TURN_POSITIONS })) {
                "frame cells need ${PetalLayout.FIELD_CELLS} turn positions of 0..3"
            }
            require(
                kanaGlyphs == null ||
                    (kanaGlyphs.size == PetalLayout.KANA_TILE_COUNT && kanaGlyphs.all { it in 0 until PetalGlyphs.GLYPH_COUNT }),
            ) { "frame cells need ${PetalLayout.KANA_TILE_COUNT} glyphs of 0..15" }
            require(kanaLight == null || kanaLight.size == PetalLayout.KANA_TILE_COUNT) {
                "frame cells need ${PetalLayout.KANA_TILE_COUNT} tile polarities"
            }
            val glyphs = ByteArray(PetalLayout.KANA_TILE_COUNT)
            val light = BooleanArray(PetalLayout.KANA_TILE_COUNT)
            pilotSymbols(glyphs, light)
            return PetalFrameCells(
                dots.copyOf(),
                petals.copyOf(),
                turns?.copyOf() ?: ByteArray(PetalLayout.FIELD_CELLS),
                kanaGlyphs?.copyOf() ?: glyphs,
                kanaLight?.copyOf() ?: light,
            )
        }

        /** Every tile light with glyph 0, then the pilots' fixed symbols. */
        internal fun pilotSymbols(glyphs: ByteArray, light: BooleanArray) {
            java.util.Arrays.fill(glyphs, 0.toByte())
            java.util.Arrays.fill(light, true)
            for (tile in 0 until PetalLayout.KANA_TILE_COUNT) {
                val pilot = PetalLayout.TILE_PILOT[tile]
                if (pilot < 0) continue
                glyphs[tile] = PetalLayout.KANA_PILOT_GLYPHS[pilot].toByte()
                light[tile] = PetalLayout.KANA_PILOT_LIGHT[pilot]
            }
        }

        /**
         * Shows the transmitted kana codewords [words] (`P0`, `K0`, `P1`, `K1`) on the data tiles:
         * data tile `i` of a band shows bit `i` of its polarity codeword (the last two tiles, beyond
         * it, are light) and nibble `i` of its glyph codeword.
         */
        internal fun setKana(glyphs: ByteArray, light: BooleanArray, words: List<ByteArray>) {
            require(words.size == PetalKanaLane.ALL.size && PetalKanaLane.ALL.all { words[it.ordinal].size == it.wordLength }) {
                "kana lane codeword length mismatch"
            }
            for (band in 0 until PetalLayout.KANA_BANDS) {
                val polarity = words[PetalKanaLane.of(band, false).ordinal]
                val glyphWord = words[PetalKanaLane.of(band, true).ordinal]
                val base = band * PetalKana.KANA_DATA_TILES
                for (i in 0 until PetalKana.KANA_DATA_TILES) {
                    val tile = PetalLayout.KANA_DATA_TILES[base + i]
                    light[tile] = if (i < PetalKana.KANA_POLARITY_WORD * 8) {
                        ((polarity[i / 8].toInt() ushr (7 - i % 8)) and 1) == 1
                    } else {
                        true
                    }
                    val byte = glyphWord[i / 2].toInt() and 0xFF
                    glyphs[tile] = (if (i % 2 == 0) byte ushr 4 else byte and 0x0F).toByte()
                }
            }
        }

        /**
         * Builds the cells from the transmitted codewords of lane `D` and the four field lanes
         * (sector 0 first), with every petal unturned (see [withTurns]) and only the pilots set
         * among the katakana tiles (see [withKana]).
         */
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
            val glyphs = ByteArray(PetalLayout.KANA_TILE_COUNT)
            val light = BooleanArray(PetalLayout.KANA_TILE_COUNT)
            pilotSymbols(glyphs, light)
            return PetalFrameCells(dots, petals, ByteArray(PetalLayout.FIELD_CELLS), glyphs, light)
        }

        /**
         * Turns the carriers of every sector in [turns] to show the transmitted turn codeword of
         * [words]: the sector's codeword cells are unturned first, and a sector with too few lit
         * petals stays so.
         */
        internal fun setTurns(petals: BooleanArray, turns: ByteArray, words: List<ByteArray>) {
            val chosen = IntArray(PetalTurns.TURN_CARRIERS)
            for (sector in 0 until PetalLayout.SECTORS) {
                val word = words[sector]
                val base = sector * PetalLayout.SECTOR_CELLS
                for (bit in 0 until PetalLanes.FIELD_BITS) turns[PetalLayout.SECTOR_ORDER[base + bit]] = 0
                if (!PetalTurns.fillCarriers(petals, sector, chosen, 0)) continue
                for (index in 0 until PetalTurns.TURN_CARRIERS) {
                    turns[chosen[index]] = PetalTurns.carrierPosition(word, index).toByte()
                }
            }
        }
    }
}
