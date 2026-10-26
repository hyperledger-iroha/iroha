package org.hyperledger.iroha.sdk.offline.petal

import java.util.Arrays

/**
 * One of the four kana lanes of a card: the polarity lane (`P0`, `P1`) and the glyph lane (`K0`,
 * `K1`) of the band above the square (0) and of the band below it (1). Each is one Reed–Solomon
 * codeword, XOR-whitened with a fixed xorshift32 sequence (see [PetalKana]).
 */
enum class PetalKanaLane(
    /** Band: 0 above the square, 1 below it. */
    val band: Int,
    /** Whether the lane carries the tiles' katakana (`K`) rather than their polarity (`P`). */
    val isGlyph: Boolean,
    /** Codeword length in bytes. */
    val wordLength: Int,
    /** Parity bytes. */
    val parityLength: Int,
    /** Atoms carried. */
    val atoms: Int,
    whiteningSeed: Int,
) {
    /** The light or dark tiles of the band above the square. */
    P0(0, false, PetalKana.KANA_POLARITY_WORD, PetalKana.KANA_POLARITY_PARITY, PetalKana.KANA_POLARITY_ATOMS, 0x504F_4C30), // "POL0"

    /** The katakana of the band above the square. */
    K0(0, true, PetalKana.KANA_GLYPH_WORD, PetalKana.KANA_GLYPH_PARITY, PetalKana.KANA_GLYPH_ATOMS, 0x4B41_4E30), // "KAN0"

    /** The light or dark tiles of the band below the square. */
    P1(1, false, PetalKana.KANA_POLARITY_WORD, PetalKana.KANA_POLARITY_PARITY, PetalKana.KANA_POLARITY_ATOMS, 0x504F_4C31), // "POL1"

    /** The katakana of the band below the square. */
    K1(1, true, PetalKana.KANA_GLYPH_WORD, PetalKana.KANA_GLYPH_PARITY, PetalKana.KANA_GLYPH_ATOMS, 0x4B41_4E31), // "KAN1"
    ;

    /** Data bytes: the atoms, then their check. */
    val dataLength: Int get() = wordLength - parityLength

    /** The byte that names the lane in its check: `0x10 | band << 1 | kind` (polarity 0, glyph 1). */
    val code: Int get() = 0x10 or (band shl 1) or (if (isGlyph) 1 else 0)

    internal val whiteningBytes: ByteArray = PetalXorshift32(whiteningSeed).let { rng ->
        ByteArray(wordLength) { rng.nextByte().toByte() }
    }

    internal val codec = PetalReedSolomon(parityLength)

    /** The fixed whitening sequence of the lane. */
    fun whitening(): ByteArray = whiteningBytes.copyOf()

    companion object {
        /** The four lanes: `P0`, `K0`, `P1`, `K1`. */
        @JvmField
        val ALL: List<PetalKanaLane> = java.util.Collections.unmodifiableList(listOf(P0, K0, P1, K1))

        /** The polarity ([glyph] false) or glyph lane of [band] (`0..1`). */
        @JvmStatic
        fun of(band: Int, glyph: Boolean): PetalKanaLane = when (band) {
            0 -> if (glyph) K0 else P0
            1 -> if (glyph) K1 else P1
            else -> throw IllegalArgumentException("band must be 0..1")
        }
    }
}

/**
 * The kana layer: data in the katakana tiles of the bands above and below the square.
 *
 * The card ([PetalLayout.CARD_HEIGHT]) is the square code with a band of katakana tiles above and
 * below it, in the part of an upright phone's camera frame that the square leaves unused. Each band
 * holds [PetalLayout.KANA_BAND_TILES] tiles ([PetalLayout.kanaTiles]); eight are pilots with fixed
 * symbols and the other [KANA_DATA_TILES] carry two lanes:
 *
 * | lane | cells | codeword | data | parity |
 * |------|-------|----------|------|--------|
 * | `P0`, `P1` polarity | 296 data tiles × 1 bit (light or dark) | 37 B | 20 B | 17 B |
 * | `K0`, `K1` glyph | 298 data tiles × 4 bits (one of sixteen katakana) | 149 B | 116 B | 33 B |
 *
 * A light tile is filled light with its glyph in ink; a dark tile is black with its glyph in pink.
 * Data tile `i` of a band (its tiles that are not pilots, in reading order) shows bit `i` of the
 * band's polarity codeword (most significant bit first; the last two data tiles, beyond the
 * codeword, are light) and nibble `i` of its glyph codeword (the high nibble of byte `i / 2` for
 * even `i`).
 *
 * Every kana lane carries fountain atoms from its own repair range ([KANA_ID_BASE], see
 * [kanaFirstId]) and a CRC-32C check that binds them to the stream tag, the frame and the lane
 * ([kanaCheck]). The lanes have no header: a reader takes the frame from the square's lanes and
 * also tries the neighbouring frames, because a rolling shutter or a stale frame can show a band
 * from the frame before or after the square.
 *
 * [readKana] reads the bands of a decoded capture. The square's pose, extended beyond the square,
 * places every tile; each band's pilots are found by a small search around their places, and an
 * affine correction fitted to their offsets moves every tile of the band. The pilots also pick the
 * blur of the glyph templates and the levels that tell light tiles from dark ones. Each data tile
 * then takes the polarity its mean level shows and the glyph whose template correlates best with
 * its samples. Atom ids are raw unsigned 32-bit patterns. All functions are thread-safe; the
 * reader's templates are built once, on first use. [PetalScanSession] reads each band once per
 * shown frame, with scratch buffers it reuses.
 */
object PetalKana {
    /** Data tiles of each band. */
    const val KANA_DATA_TILES = PetalLayout.KANA_BAND_TILES - PetalLayout.KANA_PILOTS

    /** Codeword length of a glyph lane in bytes (two data tiles per byte). */
    const val KANA_GLYPH_WORD = KANA_DATA_TILES / 2

    /** Parity bytes of a glyph lane. */
    const val KANA_GLYPH_PARITY = 33

    /** Atoms carried by each glyph lane. */
    const val KANA_GLYPH_ATOMS = 7

    /** Codeword length of a polarity lane in bytes (eight data tiles per byte). */
    const val KANA_POLARITY_WORD = KANA_DATA_TILES / 8

    /** Parity bytes of a polarity lane. */
    const val KANA_POLARITY_PARITY = 17

    /** Atoms carried by each polarity lane. */
    const val KANA_POLARITY_ATOMS = 1

    /** Bytes of the check that closes a kana lane's data. */
    const val KANA_CHECK_LEN = 4

    /**
     * Fountain id of the first kana atom of frame 0, `2³¹ + 2²⁰` as a raw unsigned 32-bit pattern:
     * just above the turn lanes' range.
     */
    const val KANA_ID_BASE = PetalTurns.TURN_ID_BASE + (1 shl 20)

    /** Kana atom ids reserved per frame (eight per band). */
    private const val KANA_IDS_PER_FRAME = 16

    /** Gaussian blurs (σ in canvas units) of the glyph templates; the pilots pick one per band. */
    @JvmField
    val KANA_BLURS: List<Double> = java.util.Collections.unmodifiableList(listOf(1.0, 1.6, 2.4, 3.4, 4.8))

    /** Samples per tile side: points `(i + ½) · 25 / 12` units from the tile's top-left corner. */
    const val KANA_GRID = 12

    /** Samples per tile side in the coarse pilot search: points `(i + ½) · 25 / 6`. */
    const val KANA_COARSE_GRID = 6

    private const val ATOM_LEN = PetalLanes.ATOM_LEN

    init {
        check(KANA_GLYPH_WORD - KANA_GLYPH_PARITY == KANA_GLYPH_ATOMS * ATOM_LEN + KANA_CHECK_LEN)
        check(KANA_POLARITY_WORD - KANA_POLARITY_PARITY == KANA_POLARITY_ATOMS * ATOM_LEN + KANA_CHECK_LEN)
        check(KANA_GLYPH_ATOMS + KANA_POLARITY_ATOMS <= KANA_IDS_PER_FRAME / PetalLayout.KANA_BANDS)
    }

    /**
     * Fountain id of the first atom [lane] carries in [frame]: `KANA_ID_BASE + 16 · frame + 8 · band`,
     * plus 7 for the polarity lane (the glyph lane's seven atoms come first).
     */
    @JvmStatic
    fun kanaFirstId(frame: Int, lane: PetalKanaLane): Int {
        PetalStream.checkFrame(frame)
        val slot = if (lane.isGlyph) 0 else KANA_GLYPH_ATOMS
        return KANA_ID_BASE + frame * KANA_IDS_PER_FRAME + lane.band * 8 + slot
    }

    /**
     * The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the lane's
     * [PetalKanaLane.code] and the [atoms].
     */
    @JvmStatic
    fun kanaCheck(header: PetalLaneHeader, lane: PetalKanaLane, atoms: ByteArray): Int =
        checkOf(header, lane, atoms, atoms.size, ByteArray(4 + atoms.size))

    /** CRC-32C of `tag, frame:u16, code, atoms[0 until length]`, using [scratch] (at least `4 + length` bytes). */
    private fun checkOf(header: PetalLaneHeader, lane: PetalKanaLane, atoms: ByteArray, length: Int, scratch: ByteArray): Int {
        scratch[0] = header.tag.toByte()
        scratch[1] = (header.frame ushr 8).toByte()
        scratch[2] = header.frame.toByte()
        scratch[3] = lane.code.toByte()
        System.arraycopy(atoms, 0, scratch, 4, length)
        return PetalCrc32c.compute(scratch, 0, 4 + length)
    }

    /** The data of a kana lane: its [PetalKanaLane.atoms] [atoms] followed by their [kanaCheck] (big-endian). */
    @JvmStatic
    fun kanaData(header: PetalLaneHeader, lane: PetalKanaLane, atoms: ByteArray): ByteArray {
        require(atoms.size == lane.atoms * ATOM_LEN) { "kana atoms length" }
        val data = atoms.copyOf(lane.dataLength)
        val check = kanaCheck(header, lane, atoms)
        for (index in 0 until KANA_CHECK_LEN) data[atoms.size + index] = (check ushr (24 - 8 * index)).toByte()
        return data
    }

    /** The atoms of kana lane [data] whose check matches [header] and [lane]; `null` otherwise. */
    @JvmStatic
    fun kanaPacket(header: PetalLaneHeader, lane: PetalKanaLane, data: ByteArray): PetalAtomPacket? {
        if (data.size != lane.dataLength) return null
        if (!checkMatches(header, lane, data, ByteArray(lane.dataLength))) return null
        return PetalAtomPacket(header, kanaFirstId(header.frame, lane), data.copyOf(lane.atoms * ATOM_LEN))
    }

    /** Whether the check closing kana lane [data] matches [header] and [lane]. */
    private fun checkMatches(header: PetalLaneHeader, lane: PetalKanaLane, data: ByteArray, scratch: ByteArray): Boolean {
        val length = lane.atoms * ATOM_LEN
        val check = checkOf(header, lane, data, length, scratch)
        for (index in 0 until KANA_CHECK_LEN) {
            if (data[length + index] != (check ushr (24 - 8 * index)).toByte()) return false
        }
        return true
    }

    /** Encodes kana lane [data] (exactly [PetalKanaLane.dataLength] bytes) into the transmitted (whitened) codeword. */
    @JvmStatic
    fun encodeKanaLane(lane: PetalKanaLane, data: ByteArray): ByteArray {
        require(data.size == lane.dataLength) { "kana data length mismatch" }
        val word = lane.codec.encode(data)
        val whitening = lane.whiteningBytes
        for (index in word.indices) word[index] = (word[index].toInt() xor whitening[index].toInt()).toByte()
        return word
    }

    /**
     * Decodes a transmitted kana codeword, returning its data with how many byte positions the
     * Reed–Solomon decoder rewrote ([PetalLaneResult.corrected]). [erasures] lists byte positions
     * the caller distrusts. The check is not verified here (see [kanaPacket]).
     *
     * @throws PetalReedSolomonException when the word is malformed or uncorrectable.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeKanaLaneCounted(lane: PetalKanaLane, transmitted: ByteArray, erasures: IntArray = IntArray(0)): PetalLaneResult =
        decodeOrNull(lane, transmitted, erasures, erasures.size)
            ?: throw PetalReedSolomonException(
                if (transmitted.size != lane.wordLength || !validErasures(lane, erasures)) {
                    PetalReedSolomonException.Reason.INVALID_SHAPE
                } else {
                    PetalReedSolomonException.Reason.UNCORRECTABLE
                },
            )

    private fun validErasures(lane: PetalKanaLane, erasures: IntArray): Boolean {
        if (erasures.size > lane.parityLength) return false
        val seen = BooleanArray(lane.wordLength)
        for (position in erasures) {
            if (position < 0 || position >= lane.wordLength || seen[position]) return false
            seen[position] = true
        }
        return true
    }

    /** [decodeKanaLaneCounted] without exceptions; uses the first [erasureCount] erasures. */
    internal fun decodeOrNull(lane: PetalKanaLane, transmitted: ByteArray, erasures: IntArray, erasureCount: Int): PetalLaneResult? {
        if (transmitted.size != lane.wordLength) return null
        val whitening = lane.whiteningBytes
        val word = ByteArray(lane.wordLength) { (transmitted[it].toInt() xor whitening[it].toInt()).toByte() }
        val corrected = lane.codec.decodeInPlace(word, erasures, erasureCount)
        if (corrected < 0) return null
        return PetalLaneResult(word.copyOf(lane.dataLength), corrected, erasureCount)
    }

    /**
     * The template value of [glyph] (a light or dark tile) under blur [blur] (an index into
     * [KANA_BLURS]) at grid point `(i, j)`, mean-centred and scaled to unit length: what
     * conformance tests compare an SDK's tables with.
     */
    @JvmStatic
    fun templateProbe(blur: Int, glyph: Int, light: Boolean, i: Int, j: Int): Double {
        require(blur in KANA_BLURS.indices) { "blur index out of range" }
        require(glyph in 0 until PetalGlyphs.GLYPH_COUNT) { "glyph index out of range" }
        require(i in 0 until KANA_GRID && j in 0 until KANA_GRID) { "grid point out of range" }
        val table = if (light) PetalKanaTables.LIGHT else PetalKanaTables.DARK
        return table[PetalKanaTables.offset(blur, glyph) + j * KANA_GRID + i]
    }

    /**
     * The coarse template value of [glyph] (a light or dark tile) at coarse grid point `(i, j)`:
     * what conformance tests compare an SDK's tables with.
     */
    @JvmStatic
    fun coarseTemplateProbe(glyph: Int, light: Boolean, i: Int, j: Int): Double {
        require(glyph in 0 until PetalGlyphs.GLYPH_COUNT) { "glyph index out of range" }
        require(i in 0 until KANA_COARSE_GRID && j in 0 until KANA_COARSE_GRID) { "grid point out of range" }
        val table = if (light) PetalKanaTables.COARSE_LIGHT else PetalKanaTables.COARSE_DARK
        return table[glyph * PetalKanaTables.COARSE_POINTS + j * KANA_COARSE_GRID + i]
    }

    /**
     * Reads the kana lanes of the bands not flagged in [skip] (two flags, band 0 first) of a
     * capture whose square [frame] decoded under [header] (the stream tag and frame its lanes
     * name). A lane passes when its Reed–Solomon codeword decodes, with 0, ⅛, ¼, ⅓ or ½ of its
     * parity bytes' worth of its least confident bytes erased, into data whose [kanaCheck] matches
     * [header]'s frame, the frame after it or the frame before it. Returns four entries in
     * [PetalKanaLane.ALL] order, `null` where no lane was read.
     */
    @JvmStatic
    @JvmOverloads
    fun readKana(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        header: PetalLaneHeader,
        skip: BooleanArray = BooleanArray(PetalLayout.KANA_BANDS),
    ): List<PetalKanaResult?> {
        require(skip.size == PetalLayout.KANA_BANDS) { "skip has one flag per band" }
        val results = arrayOfNulls<PetalKanaResult>(PetalKanaLane.ALL.size)
        PetalKanaReader().read(image, frame, header, skip, results)
        return java.util.Collections.unmodifiableList(results.toList())
    }

    /**
     * What [readKana] sees of band [band] (`0..1`) before any Reed–Solomon correction, for
     * diagnostics; `null` when fewer than two pilots are found or the pilots show no contrast
     * between light and dark tiles.
     */
    @JvmStatic
    fun observeBand(image: PetalLuma, frame: PetalDecodedFrame, band: Int): PetalBandReading? {
        require(band in 0 until PetalLayout.KANA_BANDS) { "band index out of range" }
        return PetalKanaReader().observe(image, frame, band)
    }

    /**
     * Tries Reed–Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident bytes of [word] erased
     * (n the parity bytes; a stable ranking: ties keep byte order) and returns the first result
     * whose check matches [header]'s frame, the frame after it or the frame before it. [order] and
     * [trial] are scratch (at least [PetalKanaLane.wordLength] entries).
     */
    internal fun decodeKanaWord(
        lane: PetalKanaLane,
        header: PetalLaneHeader,
        word: ByteArray,
        confidence: DoubleArray,
        order: IntArray,
        trial: ByteArray,
        checkScratch: ByteArray,
    ): PetalKanaResult? {
        val length = lane.wordLength
        // stable sort of byte positions by confidence (`sort_by` with `total_cmp`)
        for (i in 0 until length) {
            val key = confidence[i]
            var j = i - 1
            while (j >= 0 && PetalNumerics.totalCompare(confidence[order[j]], key) > 0) {
                order[j + 1] = order[j]
                j -= 1
            }
            order[j + 1] = i
        }
        val nsym = lane.parityLength
        val schedule = intArrayOf(0, nsym / 8, nsym / 4, nsym / 3, nsym / 2)
        var previous = -1
        for (erasures in schedule) {
            if (erasures == previous) continue // `dedup` of consecutive repeats
            previous = erasures
            System.arraycopy(word, 0, trial, 0, length)
            for (index in 0 until erasures) trial[order[index]] = 0
            val result = decodeOrNull(lane, trial.copyOf(length), order, erasures) ?: continue
            for (step in 0 until 3) {
                // the square's frame, the frame after it, the frame before it
                val candidate = when (step) {
                    0 -> header
                    1 -> PetalLaneHeader(header.tag, (header.frame + 1) and PetalStream.MAX_FRAME)
                    else -> PetalLaneHeader(header.tag, (header.frame - 1) and PetalStream.MAX_FRAME)
                }
                if (checkMatches(candidate, lane, result.dataView, checkScratch)) return PetalKanaResult(result, candidate)
            }
        }
        return null
    }
}

/** A kana lane read from a capture, with the header (stream tag and frame) its check matched. */
class PetalKanaResult internal constructor(
    /** The lane data (atoms and check). */
    val result: PetalLaneResult,
    /** The header the check matched: the square's frame or a neighbouring one. */
    val header: PetalLaneHeader,
) {
    override fun equals(other: Any?): Boolean = other is PetalKanaResult && result == other.result && header == other.header

    override fun hashCode(): Int = 31 * result.hashCode() + header.hashCode()
}

/** What a reader decided for one data tile (see [PetalKana.observeBand]). */
class PetalTileReading internal constructor(
    /** Index into [PetalLayout.kanaTiles]. */
    val tile: Int,
    /** Decided polarity. */
    val light: Boolean,
    /** How far the tile's mean level lay from the threshold, in half-contrasts. */
    val lightMargin: Double,
    /** Decided glyph (`0..15`). */
    val glyph: Int,
    /** How far the glyph's correlation led the runner-up's. */
    val glyphLead: Double,
) {
    override fun toString(): String =
        "PetalTileReading(tile=$tile, light=$light, lightMargin=$lightMargin, glyph=$glyph, glyphLead=$glyphLead)"
}

/** What a reader saw of one band (see [PetalKana.observeBand]). */
class PetalBandReading internal constructor(
    /** The blur (σ in canvas units) the pilots chose. */
    val blur: Double,
    /** Pilots found. */
    val pilots: Int,
    /** Every data tile of the band in data order, or `null` for tiles it could not read. */
    val tiles: List<PetalTileReading?>,
)

/**
 * The kana reader's templates, built once on first use (class initialisation is lazy and
 * thread-safe): for every blur of [PetalKana.KANA_BLURS], the coverage rasters of the tile's inside
 * and of every glyph's ink, blurred and sampled at the [PetalKana.KANA_GRID] points and
 * mean-centred; a light tile's template is the inside less the ink, a dark tile's the ink, each also
 * scaled to unit length. The coarse templates are the same under blur 3.4 at the
 * [PetalKana.KANA_COARSE_GRID] points.
 */
internal object PetalKanaTables {
    /** Samples of a tile. */
    const val POINTS = PetalKana.KANA_GRID * PetalKana.KANA_GRID

    /** Samples of a tile in the coarse pilot search. */
    const val COARSE_POINTS = PetalKana.KANA_COARSE_GRID * PetalKana.KANA_COARSE_GRID

    /** The blur of the coarse templates (3.4 units). */
    const val COARSE_BLUR = 3

    /** Template raster step in canvas units. */
    private const val RASTER_STEP = 0.5

    /** The raster starts this far outside the tile square, so blur reaches in from the gutter. */
    private const val RASTER_MARGIN = 4.0

    /** Raster side in steps. */
    private const val RASTER_SIDE = 67

    private const val GLYPHS = PetalGlyphs.GLYPH_COUNT

    /** Tile-local coordinate of every grid index on either axis: `(i + ½) · 25 / 12` … */
    @JvmField
    val GRID = DoubleArray(PetalKana.KANA_GRID)

    /** … and of the coarse grid: `(i + ½) · 25 / 6`. */
    @JvmField
    val COARSE_GRID = DoubleArray(PetalKana.KANA_COARSE_GRID)

    /** Per blur: the tile's inside at the grid points, mean-centred. */
    @JvmField
    val INSIDE = DoubleArray(PetalKana.KANA_BLURS.size * POINTS)

    /** Per blur and glyph (at [offset]): the ink at the grid points, mean-centred. */
    @JvmField
    val INK = DoubleArray(PetalKana.KANA_BLURS.size * GLYPHS * POINTS)

    /** Per blur and glyph (`blur · 16 + glyph`): the lengths of the light (inside less ink) and dark (ink) templates. */
    @JvmField
    val LIGHT_NORM = DoubleArray(PetalKana.KANA_BLURS.size * GLYPHS)

    @JvmField
    val DARK_NORM = DoubleArray(LIGHT_NORM.size)

    /** Per blur and glyph (at [offset]): the light and dark templates scaled to unit length. */
    @JvmField
    val LIGHT = DoubleArray(INK.size)

    @JvmField
    val DARK = DoubleArray(INK.size)

    /** Per glyph (`glyph · 36`): the coarse light and dark templates, unit length. */
    @JvmField
    val COARSE_LIGHT = DoubleArray(GLYPHS * COARSE_POINTS)

    @JvmField
    val COARSE_DARK = DoubleArray(COARSE_LIGHT.size)

    init {
        for (i in 0 until PetalKana.KANA_GRID) GRID[i] = gridPoint(i, PetalKana.KANA_GRID)
        for (i in 0 until PetalKana.KANA_COARSE_GRID) COARSE_GRID[i] = gridPoint(i, PetalKana.KANA_COARSE_GRID)
        val inside = raster(-1)
        val inks = Array(GLYPHS) { raster(it) }
        val pass = DoubleArray(RASTER_SIDE * RASTER_SIDE)
        val insideBlurred = DoubleArray(RASTER_SIDE * RASTER_SIDE)
        val inkBlurred = DoubleArray(RASTER_SIDE * RASTER_SIDE)
        val light = DoubleArray(POINTS)
        val coarseInside = DoubleArray(COARSE_POINTS)
        val coarseInk = DoubleArray(COARSE_POINTS)
        val coarseLight = DoubleArray(COARSE_POINTS)
        for (blur in PetalKana.KANA_BLURS.indices) {
            val sigma = PetalKana.KANA_BLURS[blur]
            blurRaster(inside, sigma, pass, insideBlurred)
            sampleRaster(insideBlurred, PetalKana.KANA_GRID, INSIDE, blur * POINTS)
            for (glyph in 0 until GLYPHS) {
                blurRaster(inks[glyph], sigma, pass, inkBlurred)
                val at = offset(blur, glyph)
                sampleRaster(inkBlurred, PetalKana.KANA_GRID, INK, at)
                for (p in 0 until POINTS) light[p] = INSIDE[blur * POINTS + p] - INK[at + p]
                val lightNorm = norm(light, 0, POINTS)
                val darkNorm = norm(INK, at, POINTS)
                LIGHT_NORM[blur * GLYPHS + glyph] = lightNorm
                DARK_NORM[blur * GLYPHS + glyph] = darkNorm
                for (p in 0 until POINTS) {
                    LIGHT[at + p] = light[p] / lightNorm
                    DARK[at + p] = INK[at + p] / darkNorm
                }
                if (blur == COARSE_BLUR) {
                    sampleRaster(insideBlurred, PetalKana.KANA_COARSE_GRID, coarseInside, 0)
                    sampleRaster(inkBlurred, PetalKana.KANA_COARSE_GRID, coarseInk, 0)
                    for (p in 0 until COARSE_POINTS) coarseLight[p] = coarseInside[p] - coarseInk[p]
                    val coarseLightNorm = norm(coarseLight, 0, COARSE_POINTS)
                    val coarseDarkNorm = norm(coarseInk, 0, COARSE_POINTS)
                    for (p in 0 until COARSE_POINTS) {
                        COARSE_LIGHT[glyph * COARSE_POINTS + p] = coarseLight[p] / coarseLightNorm
                        COARSE_DARK[glyph * COARSE_POINTS + p] = coarseInk[p] / coarseDarkNorm
                    }
                }
            }
        }
    }

    /** Offset of the templates of [glyph] under blur [blur] in [INK], [LIGHT] and [DARK]. */
    fun offset(blur: Int, glyph: Int): Int = (blur * GLYPHS + glyph) * POINTS

    /** Tile-local coordinate of grid index [i] on either axis of a grid of [side] points. */
    private fun gridPoint(i: Int, side: Int): Double = (i.toDouble() + 0.5) * PetalLayout.KANA_TILE / side.toDouble()

    /**
     * Coverage on the template raster of the tile's inside ([glyph] `-1`) or of [glyph]'s ink:
     * raster point `a` is at `−4 + a / 2` units; each value is the share of its 4 × 4 sub-points
     * `(m − 1.5) / 8` units around it that are covered.
     */
    private fun raster(glyph: Int): DoubleArray {
        val table = DoubleArray(RASTER_SIDE * RASTER_SIDE)
        for (b in 0 until RASTER_SIDE) {
            for (a in 0 until RASTER_SIDE) {
                val u0 = -RASTER_MARGIN + a.toDouble() * RASTER_STEP
                val v0 = -RASTER_MARGIN + b.toDouble() * RASTER_STEP
                var hits = 0
                for (n in 0 until 4) {
                    for (m in 0 until 4) {
                        val u = u0 + (m.toDouble() - 1.5) / 8.0
                        val v = v0 + (n.toDouble() - 1.5) / 8.0
                        val covered = if (glyph < 0) PetalLayout.kanaInside(u, v) else PetalLayout.inked(glyph, u, v)
                        if (covered) hits += 1
                    }
                }
                table[b * RASTER_SIDE + a] = hits.toDouble() / 16.0
            }
        }
        return table
    }

    /**
     * Separable Gaussian blur of a raster into [out] ([pass] is scratch): taps every
     * [RASTER_STEP] out to `⌈3σ / step⌉`, weights `exp(−d² / (2σ²))` normalised by their sum,
     * points outside counting as zero; rows first.
     */
    private fun blurRaster(table: DoubleArray, sigma: Double, pass: DoubleArray, out: DoubleArray) {
        val reach = Math.ceil(3.0 * sigma / RASTER_STEP).toInt()
        val weights = DoubleArray(2 * reach + 1)
        for (k in -reach..reach) {
            val d = k.toDouble() * RASTER_STEP
            weights[k + reach] = StrictMath.exp(-(d * d) / (2.0 * sigma * sigma))
        }
        var sum = 0.0
        for (weight in weights) sum += weight
        for (index in weights.indices) weights[index] = weights[index] / sum
        blurPass(table, pass, weights, reach, true)
        blurPass(pass, out, weights, reach, false)
    }

    private fun blurPass(source: DoubleArray, out: DoubleArray, weights: DoubleArray, reach: Int, alongRows: Boolean) {
        for (b in 0 until RASTER_SIDE) {
            for (a in 0 until RASTER_SIDE) {
                var acc = 0.0
                for (k in -reach..reach) {
                    val aa = if (alongRows) a + k else a
                    val bb = if (alongRows) b else b + k
                    if (aa < 0 || bb < 0 || aa >= RASTER_SIDE || bb >= RASTER_SIDE) continue
                    acc += weights[k + reach] * source[bb * RASTER_SIDE + aa]
                }
                out[b * RASTER_SIDE + a] = acc
            }
        }
    }

    /** A blurred raster at the [side] × [side] grid points (bilinear) into `out[offset …]`, mean-centred. */
    private fun sampleRaster(table: DoubleArray, side: Int, out: DoubleArray, offset: Int) {
        for (j in 0 until side) {
            for (i in 0 until side) {
                val fa = (gridPoint(i, side) + RASTER_MARGIN) / RASTER_STEP
                val fb = (gridPoint(j, side) + RASTER_MARGIN) / RASTER_STEP
                val a0 = Math.floor(fa).toInt()
                val b0 = Math.floor(fb).toInt()
                val ta = fa - a0.toDouble()
                val tb = fb - b0.toDouble()
                val top = table[b0 * RASTER_SIDE + a0] * (1.0 - ta) + table[b0 * RASTER_SIDE + a0 + 1] * ta
                val bottom = table[(b0 + 1) * RASTER_SIDE + a0] * (1.0 - ta) + table[(b0 + 1) * RASTER_SIDE + a0 + 1] * ta
                out[offset + j * side + i] = top * (1.0 - tb) + bottom * tb
            }
        }
        val n = side * side
        var sum = 0.0
        for (p in 0 until n) sum += out[offset + p]
        val mean = sum / n.toDouble()
        for (p in 0 until n) out[offset + p] = out[offset + p] - mean
    }

    /** The length of `values[offset until offset + count]`, at least `1e-12`. */
    private fun norm(values: DoubleArray, offset: Int, count: Int): Double {
        var sum = 0.0
        for (p in 0 until count) sum += values[offset + p] * values[offset + p]
        return PetalNumerics.max(Math.sqrt(sum), 1e-12)
    }
}

/**
 * The kana reader with its scratch buffers (`observe_band` and `read_kana` of the reference),
 * reused between captures. Not thread-safe: one reader per decoding thread (a [PetalWorkspace]
 * holds one).
 */
internal class PetalKanaReader {
    private companion object {
        const val BANDS = PetalLayout.KANA_BANDS
        const val PILOTS = PetalLayout.KANA_PILOTS
        const val DATA = PetalKana.KANA_DATA_TILES
        const val POINTS = PetalKanaTables.POINTS
        const val GRID_SIDE = PetalKana.KANA_GRID

        /** Step of the coarse pilot search in canvas units. */
        const val COARSE_STEP = 2.0

        /** A pilot whose coarse correlation is below this is not followed further. */
        const val PILOT_COARSE_MIN = 0.35

        /** A pilot whose final correlation is below this is not used. */
        const val PILOT_MIN = 0.5

        val REFINE_COARSE = doubleArrayOf(1.0)
        val REFINE_FINE = doubleArrayOf(1.0, 0.5, 0.25)
    }

    private var image: PetalLuma? = null
    private var pose: PetalPose? = null
    private val point = DoubleArray(6)
    private val samples = DoubleArray(POINTS)

    /** The samples, mean and energy of the last [sampleTile]. */
    private var count = 0
    private var mean = 0.0
    private var energy = 0.0

    // the best shift of the last search and its correlation
    private var bestX = 0.0
    private var bestY = 0.0
    private var bestScore = 0.0

    // pilots: kept after the coarse search, then found
    private val coarseTile = IntArray(PILOTS)
    private val coarseX = DoubleArray(PILOTS)
    private val coarseY = DoubleArray(PILOTS)
    private var coarseCount = 0
    private val foundCentreX = DoubleArray(PILOTS)
    private val foundCentreY = DoubleArray(PILOTS)
    private val foundShiftX = DoubleArray(PILOTS)
    private val foundShiftY = DoubleArray(PILOTS)
    private val foundLevel = DoubleArray(PILOTS)
    private val foundLight = BooleanArray(PILOTS)
    private var foundCount = 0

    // shift field and level lines
    private val fieldX = DoubleArray(3)
    private val fieldY = DoubleArray(3)
    private var fieldCentre = 0.0
    private val normal = Array(3) { DoubleArray(3) }
    private val solveMatrix = Array(3) { DoubleArray(3) }
    private val rightX = DoubleArray(3)
    private val rightY = DoubleArray(3)
    private val solveRight = DoubleArray(3)
    private val solution = DoubleArray(3)
    private val basis = DoubleArray(3)
    private val lineX = DoubleArray(PILOTS)
    private val lineY = DoubleArray(PILOTS)
    private var lightIntercept = 0.0
    private var lightSlope = 0.0
    private var darkIntercept = 0.0
    private var darkSlope = 0.0
    private var lineIntercept = 0.0
    private var lineSlope = 0.0

    // the band's readings, in data order
    private var blur = 0
    private val tileRead = BooleanArray(DATA)
    private val tileLight = BooleanArray(DATA)
    private val tileMargin = DoubleArray(DATA)
    private val tileGlyph = IntArray(DATA)
    private val tileLead = DoubleArray(DATA)
    private val scores = DoubleArray(PetalGlyphs.GLYPH_COUNT)

    // codewords
    private val polarity = ByteArray(PetalKana.KANA_POLARITY_WORD)
    private val polarityConfidence = DoubleArray(PetalKana.KANA_POLARITY_WORD)
    private val glyphs = ByteArray(PetalKana.KANA_GLYPH_WORD)
    private val glyphConfidence = DoubleArray(PetalKana.KANA_GLYPH_WORD)
    private val order = IntArray(PetalKana.KANA_GLYPH_WORD)
    private val trial = ByteArray(PetalKana.KANA_GLYPH_WORD)
    private val checkScratch = ByteArray(4 + PetalKana.KANA_GLYPH_ATOMS * PetalLanes.ATOM_LEN)

    /** [PetalKana.readKana] into [results] (four entries in [PetalKanaLane.ALL] order, cleared first). */
    fun read(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        header: PetalLaneHeader,
        skip: BooleanArray,
        results: Array<PetalKanaResult?>,
    ) {
        Arrays.fill(results, null)
        for (band in 0 until BANDS) {
            if (skip[band]) continue
            if (!observeInto(image, frame, band)) continue
            Arrays.fill(polarity, 0.toByte())
            Arrays.fill(polarityConfidence, Double.MAX_VALUE)
            Arrays.fill(glyphs, 0.toByte())
            Arrays.fill(glyphConfidence, Double.MAX_VALUE)
            for (i in 0 until DATA) {
                val read = tileRead[i]
                val light = read && tileLight[i]
                val margin = if (read) tileMargin[i] else 0.0
                val glyph = if (read) tileGlyph[i] else 0
                val lead = if (read) tileLead[i] else 0.0
                if (i < PetalKana.KANA_POLARITY_WORD * 8) {
                    if (light) polarity[i / 8] = (polarity[i / 8].toInt() or (1 shl (7 - i % 8))).toByte()
                    polarityConfidence[i / 8] = PetalNumerics.min(polarityConfidence[i / 8], margin)
                }
                val nibble = if (i % 2 == 0) glyph shl 4 else glyph
                glyphs[i / 2] = (glyphs[i / 2].toInt() or nibble).toByte()
                glyphConfidence[i / 2] = PetalNumerics.min(glyphConfidence[i / 2], lead)
            }
            val polarityLane = PetalKanaLane.of(band, false)
            val glyphLane = PetalKanaLane.of(band, true)
            results[2 * band] = PetalKana.decodeKanaWord(polarityLane, header, polarity, polarityConfidence, order, trial, checkScratch)
            results[2 * band + 1] = PetalKana.decodeKanaWord(glyphLane, header, glyphs, glyphConfidence, order, trial, checkScratch)
        }
        this.image = null
        this.pose = null
    }

    /** [PetalKana.observeBand]. */
    fun observe(image: PetalLuma, frame: PetalDecodedFrame, band: Int): PetalBandReading? {
        if (!observeInto(image, frame, band)) return null
        val tiles = ArrayList<PetalTileReading?>(DATA)
        for (i in 0 until DATA) {
            tiles += if (tileRead[i]) {
                PetalTileReading(PetalLayout.KANA_DATA_TILES[band * DATA + i], tileLight[i], tileMargin[i], tileGlyph[i], tileLead[i])
            } else {
                null
            }
        }
        this.image = null
        this.pose = null
        return PetalBandReading(PetalKana.KANA_BLURS[blur], foundCount, java.util.Collections.unmodifiableList(tiles))
    }

    /**
     * Reads band [band]'s data tiles into the tile arrays; `false` when fewer than two pilots are
     * found or the pilots show no contrast between light and dark tiles.
     */
    private fun observeInto(image: PetalLuma, frame: PetalDecodedFrame, band: Int): Boolean {
        if (image.width <= 0 || image.height <= 0) return false
        this.image = image
        this.pose = frame.pose
        // coarse search of every pilot in view
        coarseCount = 0
        for (p in 0 until PILOTS) {
            val tile = PetalLayout.KANA_PILOT_TILES[band * PILOTS + p]
            if (!tileInImage(tile)) continue
            coarsePilot(tile)
            if (bestScore >= PILOT_COARSE_MIN) {
                coarseTile[coarseCount] = tile
                coarseX[coarseCount] = bestX
                coarseY[coarseCount] = bestY
                coarseCount += 1
            }
        }
        if (coarseCount < 2) return false
        // the blur under which the pilots, each at its best shift within a unit, correlate best
        blur = 0
        var best = -Double.MAX_VALUE
        for (index in PetalKana.KANA_BLURS.indices) {
            var sum = 0.0
            for (c in 0 until coarseCount) {
                refinePilot(coarseTile[c], index, coarseX[c], coarseY[c], REFINE_COARSE)
                sum += bestScore
            }
            if (sum > best) {
                best = sum
                blur = index
            }
        }
        foundCount = 0
        for (c in 0 until coarseCount) {
            val tile = coarseTile[c]
            refinePilot(tile, blur, coarseX[c], coarseY[c], REFINE_FINE)
            if (bestScore < PILOT_MIN) continue
            val shiftX = bestX
            val shiftY = bestY
            sampleTile(PetalLayout.TILE_X[tile], PetalLayout.TILE_Y[tile], shiftX, shiftY, PetalKanaTables.GRID)
            val half = PetalLayout.KANA_TILE / 2.0
            foundCentreX[foundCount] = PetalLayout.TILE_X[tile] + half
            foundCentreY[foundCount] = PetalLayout.TILE_Y[tile] + half
            foundShiftX[foundCount] = shiftX
            foundShiftY[foundCount] = shiftY
            foundLevel[foundCount] = mean
            foundLight[foundCount] = PetalLayout.KANA_PILOT_LIGHT[PetalLayout.TILE_PILOT[tile]]
            foundCount += 1
        }
        if (foundCount < 2) return false
        fitShiftField()
        if (!levelLine(true)) return false
        lightIntercept = lineIntercept
        lightSlope = lineSlope
        if (!levelLine(false)) return false
        darkIntercept = lineIntercept
        darkSlope = lineSlope
        val inside = PetalKanaTables.INSIDE
        val ink = PetalKanaTables.INK
        for (i in 0 until DATA) {
            val tile = PetalLayout.KANA_DATA_TILES[band * DATA + i]
            tileRead[i] = false
            if (!tileInImage(tile)) continue
            val x0 = PetalLayout.TILE_X[tile]
            val y0 = PetalLayout.TILE_Y[tile]
            val half = PetalLayout.KANA_TILE / 2.0
            val centreX = x0 + half
            val centreY = y0 + half
            shiftAt(centreX, centreY)
            sampleTile(x0, y0, solution[0], solution[1], PetalKanaTables.GRID)
            val x = (centreX - PetalLayout.CANVAS / 2.0) / 512.0
            val high = lightIntercept + lightSlope * x
            val low = darkIntercept + darkSlope * x
            val halfContrast = (high - low) / 2.0
            if (halfContrast <= 1.0) continue
            val threshold = (high + low) / 2.0
            // a light template is the inside less the ink, so one dot product with the inside and
            // one with each glyph's ink give both polarities' correlations
            java.util.Arrays.fill(scores, 0.0)
            if (energy > 1e-9) {
                val root = Math.sqrt(energy)
                val insideDot = dot(inside, blur * POINTS)
                for (g in 0 until PetalGlyphs.GLYPH_COUNT) {
                    val inkDot = dot(ink, PetalKanaTables.offset(blur, g))
                    val norms = blur * PetalGlyphs.GLYPH_COUNT + g
                    val light = (insideDot - inkDot) / PetalKanaTables.LIGHT_NORM[norms] / root
                    val dark = inkDot / PetalKanaTables.DARK_NORM[norms] / root
                    scores[g] = PetalNumerics.max(light, dark)
                }
            }
            var first = 0
            for (g in 1 until PetalGlyphs.GLYPH_COUNT) if (scores[g] > scores[first]) first = g
            var second = -Double.MAX_VALUE
            for (g in 0 until PetalGlyphs.GLYPH_COUNT) if (g != first) second = PetalNumerics.max(second, scores[g])
            tileRead[i] = true
            tileLight[i] = mean > threshold
            tileMargin[i] = Math.abs(mean - threshold) / halfContrast
            tileGlyph[i] = first
            tileLead[i] = scores[first] - second
        }
        return true
    }

    /**
     * Coarse search of a pilot: shifts on a [COARSE_STEP] grid within its reach, sampled on the
     * coarse grid under the coarse template; leaves the best shift in [bestX], [bestY] and its
     * correlation in [bestScore].
     */
    private fun coarsePilot(tile: Int) {
        val pilot = PetalLayout.TILE_PILOT[tile]
        val glyph = PetalLayout.KANA_PILOT_GLYPHS[pilot]
        val template = if (PetalLayout.KANA_PILOT_LIGHT[pilot]) PetalKanaTables.COARSE_LIGHT else PetalKanaTables.COARSE_DARK
        val offset = glyph * PetalKanaTables.COARSE_POINTS
        val steps = Math.floor(pilotReach(PetalLayout.TILE_Y[tile] + PetalLayout.KANA_TILE / 2.0) / COARSE_STEP).toInt()
        var bx = 0.0
        var by = 0.0
        var best = -Double.MAX_VALUE
        for (dy in -steps..steps) {
            for (dx in -steps..steps) {
                val sx = COARSE_STEP * dx.toDouble()
                val sy = COARSE_STEP * dy.toDouble()
                sampleTile(PetalLayout.TILE_X[tile], PetalLayout.TILE_Y[tile], sx, sy, PetalKanaTables.COARSE_GRID)
                val score = correlate(template, offset)
                if (score > best) {
                    bx = sx
                    by = sy
                    best = score
                }
            }
        }
        bestX = bx
        bestY = by
        bestScore = best
    }

    /**
     * Refines a pilot's shift under [blurIndex]: the best of the shifts one step around it, for
     * steps of [steps] in turn (ties keep the earlier); the result is left in [bestX], [bestY] and
     * [bestScore].
     */
    private fun refinePilot(tile: Int, blurIndex: Int, startX: Double, startY: Double, steps: DoubleArray) {
        var bx = startX
        var by = startY
        var best = pilotScore(tile, blurIndex, startX, startY)
        for (step in steps) {
            val centreX = bx
            val centreY = by
            for (dy in -1..1) {
                for (dx in -1..1) {
                    if (dx == 0 && dy == 0) continue
                    val sx = centreX + step * dx.toDouble()
                    val sy = centreY + step * dy.toDouble()
                    val score = pilotScore(tile, blurIndex, sx, sy)
                    if (score > best) {
                        bx = sx
                        by = sy
                        best = score
                    }
                }
            }
        }
        bestX = bx
        bestY = by
        bestScore = best
    }

    /** The correlation of a pilot tile's samples, moved by `(sx, sy)`, with its template under [blurIndex]. */
    private fun pilotScore(tile: Int, blurIndex: Int, sx: Double, sy: Double): Double {
        val pilot = PetalLayout.TILE_PILOT[tile]
        sampleTile(PetalLayout.TILE_X[tile], PetalLayout.TILE_Y[tile], sx, sy, PetalKanaTables.GRID)
        val offset = PetalKanaTables.offset(blurIndex, PetalLayout.KANA_PILOT_GLYPHS[pilot])
        return correlate(if (PetalLayout.KANA_PILOT_LIGHT[pilot]) PetalKanaTables.LIGHT else PetalKanaTables.DARK, offset)
    }

    /**
     * Samples the tile whose top-left corner is `(x, y)` (square coordinates), moved by
     * `(sx, sy)`, at the [grid] × [grid] points into [samples]: the pose maps the tile's centre `o`
     * and the points one unit to its right and below it (`o + u`, `o + v`), and grid point `(a, b)`
     * from the centre is sampled at `(o + b·v) + a·u`. The samples are left minus their mean; the
     * mean goes to [mean] and their energy (the sum of their squares) to [energy].
     */
    private fun sampleTile(x: Double, y: Double, sx: Double, sy: Double, grid: DoubleArray) {
        val image = image!!
        val pose = pose!!
        val p = point
        val half = PetalLayout.KANA_TILE / 2.0
        val cx = x + half + sx
        val cy = y + half + sy
        pose.applyInto(cx, cy, p, 0)
        pose.applyInto(cx + 1.0, cy, p, 2)
        pose.applyInto(cx, cy + 1.0, p, 4)
        val ux = p[2] - p[0]
        val uy = p[3] - p[1]
        val vx = p[4] - p[0]
        val vy = p[5] - p[1]
        // `PetalLuma.sample`, inlined: bilinear between pixel centres, clamped to the border
        val pixels = image.pixels
        val width = image.width
        val maxX = (width - 1).toDouble()
        val maxY = (image.height - 1).toDouble()
        val lastX = width - 1
        val lastY = image.height - 1
        val side = grid.size
        var sum = 0.0
        var index = 0
        for (j in 0 until side) {
            val b = grid[j] - half
            val rowX = p[0] + b * vx
            val rowY = p[1] + b * vy
            for (i in 0 until side) {
                val a = grid[i] - half
                var fx = rowX + a * ux - 0.5
                if (fx < 0.0) fx = 0.0
                if (fx > maxX) fx = maxX
                var fy = rowY + a * uy - 0.5
                if (fy < 0.0) fy = 0.0
                if (fy > maxY) fy = maxY
                // fx and fy are not negative (or NaN, which truncates to 0 as floor does)
                val x0 = fx.toInt()
                val y0 = fy.toInt()
                val x1 = if (x0 < lastX) x0 + 1 else lastX
                val y1 = if (y0 < lastY) y0 + 1 else lastY
                val tx = fx - x0.toDouble()
                val ty = fy - y0.toDouble()
                val row0 = y0 * width
                val row1 = y1 * width
                val top = (pixels[row0 + x0].toInt() and 0xFF).toDouble() * (1.0 - tx) + (pixels[row0 + x1].toInt() and 0xFF).toDouble() * tx
                val bottom = (pixels[row1 + x0].toInt() and 0xFF).toDouble() * (1.0 - tx) + (pixels[row1 + x1].toInt() and 0xFF).toDouble() * tx
                val value = top * (1.0 - ty) + bottom * ty
                samples[index] = value
                sum += value
                index += 1
            }
        }
        val m = sum / index.toDouble()
        var e = 0.0
        for (k in 0 until index) {
            val centred = samples[k] - m
            samples[k] = centred
            e += centred * centred
        }
        count = index
        mean = m
        energy = e
    }

    /** The dot product of the last [samples] with `templates[offset …]`. */
    private fun dot(templates: DoubleArray, offset: Int): Double {
        var sum = 0.0
        for (k in 0 until count) sum += samples[k] * templates[offset + k]
        return sum
    }

    /** Correlation of the last [samples] (mean-centred, energy [energy]) with the unit, mean-centred template at `templates[offset …]`. */
    private fun correlate(templates: DoubleArray, offset: Int): Double =
        if (energy <= 1e-9) 0.0 else dot(templates, offset) / Math.sqrt(energy)

    /** Whether every corner of [tile] maps inside the image. */
    private fun tileInImage(tile: Int): Boolean {
        val image = image!!
        val pose = pose!!
        val x = PetalLayout.TILE_X[tile]
        val y = PetalLayout.TILE_Y[tile]
        val side = PetalLayout.KANA_TILE
        val width = image.width.toDouble()
        val height = image.height.toDouble()
        for (corner in 0 until 4) {
            val u = if (corner % 2 == 1) side else 0.0
            val v = if (corner >= 2) side else 0.0
            pose.applyInto(x + u, y + v, point, 0)
            val px = point[0]
            val py = point[1]
            if (!(px >= 0.0 && py >= 0.0 && px < width && py < height)) return false
        }
        return true
    }

    /**
     * How far (canvas units) the pose can miss a pilot centred `y` units down the square (negative
     * above it): 4 units at the square's edge, growing by 10 over a band's 290-unit height.
     */
    private fun pilotReach(y: Double): Double {
        val beyond = if (y < 0.0) -y else y - PetalLayout.CANVAS
        return 4.0 + 10.0 * PetalNumerics.min(PetalNumerics.max(beyond, 0.0) / 290.0, 1.2)
    }

    /**
     * Fits the band's shift field to the found pilots: per axis an affine function of the tile
     * centre (`x` scaled by 512 around the square's middle, `y` by 145 around the pilots' mean) by
     * least squares, or their mean shift when fewer than three pilots fit one.
     */
    private fun fitShiftField() {
        val n = foundCount.toDouble()
        var sumY = 0.0
        for (p in 0 until foundCount) sumY += foundCentreY[p]
        fieldCentre = sumY / n
        var meanX = 0.0
        for (p in 0 until foundCount) meanX += foundShiftX[p]
        var meanY = 0.0
        for (p in 0 until foundCount) meanY += foundShiftY[p]
        fieldX[0] = meanX / n
        fieldX[1] = 0.0
        fieldX[2] = 0.0
        fieldY[0] = meanY / n
        fieldY[1] = 0.0
        fieldY[2] = 0.0
        if (foundCount < 3) return
        for (i in 0 until 3) {
            Arrays.fill(normal[i], 0.0)
            rightX[i] = 0.0
            rightY[i] = 0.0
        }
        for (p in 0 until foundCount) {
            basis[0] = 1.0
            basis[1] = (foundCentreX[p] - PetalLayout.CANVAS / 2.0) / 512.0
            basis[2] = (foundCentreY[p] - fieldCentre) / 145.0
            for (i in 0 until 3) {
                for (j in 0 until 3) normal[i][j] += basis[i] * basis[j]
                rightX[i] += basis[i] * foundShiftX[p]
                rightY[i] += basis[i] * foundShiftY[p]
            }
        }
        if (!solve3(rightX)) return
        val x0 = solution[0]
        val x1 = solution[1]
        val x2 = solution[2]
        if (!solve3(rightY)) return
        fieldX[0] = x0
        fieldX[1] = x1
        fieldX[2] = x2
        fieldY[0] = solution[0]
        fieldY[1] = solution[1]
        fieldY[2] = solution[2]
    }

    /** The shift of the field at the tile centre `(x, y)`, into `solution[0]`, `solution[1]`. */
    private fun shiftAt(x: Double, y: Double) {
        val b1 = (x - PetalLayout.CANVAS / 2.0) / 512.0
        val b2 = (y - fieldCentre) / 145.0
        val sx = fieldX[0] * 1.0 + fieldX[1] * b1 + fieldX[2] * b2
        val sy = fieldY[0] * 1.0 + fieldY[1] * b1 + fieldY[2] * b2
        solution[0] = sx
        solution[1] = sy
    }

    /**
     * Solves `normal · s = right` by Gauss–Jordan elimination with partial pivoting into
     * [solution]; `false` when a pivot is below `1e-9`. Of equal pivots the last wins, as Rust's
     * `max_by` picks.
     */
    private fun solve3(right: DoubleArray): Boolean {
        val m = solveMatrix
        val r = solveRight
        for (i in 0 until 3) {
            System.arraycopy(normal[i], 0, m[i], 0, 3)
            r[i] = right[i]
        }
        for (col in 0 until 3) {
            var pivot = col
            for (row in col + 1 until 3) {
                if (PetalNumerics.totalCompare(Math.abs(m[row][col]), Math.abs(m[pivot][col])) >= 0) pivot = row
            }
            if (Math.abs(m[pivot][col]) < 1e-9) return false
            val swapRow = m[col]
            m[col] = m[pivot]
            m[pivot] = swapRow
            val swapValue = r[col]
            r[col] = r[pivot]
            r[pivot] = swapValue
            for (row in 0 until 3) {
                if (row == col) continue
                val factor = m[row][col] / m[col][col]
                val pivotRow = m[col]
                for (k in col until 3) m[row][k] -= factor * pivotRow[k]
                r[row] -= factor * r[col]
            }
        }
        solution[0] = r[0] / m[0][0]
        solution[1] = r[1] / m[1][1]
        solution[2] = r[2] / m[2][2]
        return true
    }

    /**
     * A level that varies linearly across the band, fitted by least squares to the levels of the
     * light ([light]) or dark found pilots against `(x − 512) / 512`; a constant when the pilots do
     * not spread. Leaves it in [lineIntercept] and [lineSlope]; `false` without such pilots.
     */
    private fun levelLine(light: Boolean): Boolean {
        var count = 0
        for (p in 0 until foundCount) {
            if (foundLight[p] != light) continue
            lineX[count] = (foundCentreX[p] - PetalLayout.CANVAS / 2.0) / 512.0
            lineY[count] = foundLevel[p]
            count += 1
        }
        if (count == 0) return false
        val n = count.toDouble()
        var sx = 0.0
        for (p in 0 until count) sx += lineX[p]
        val mx = sx / n
        var sy = 0.0
        for (p in 0 until count) sy += lineY[p]
        val my = sy / n
        var sxx = 0.0
        for (p in 0 until count) sxx += (lineX[p] - mx) * (lineX[p] - mx)
        if (sxx < 1e-6) {
            lineIntercept = my
            lineSlope = 0.0
            return true
        }
        var sxy = 0.0
        for (p in 0 until count) sxy += (lineX[p] - mx) * (lineY[p] - my)
        val slope = sxy / sxx
        lineIntercept = my - slope * mx
        lineSlope = slope
        return true
    }
}
