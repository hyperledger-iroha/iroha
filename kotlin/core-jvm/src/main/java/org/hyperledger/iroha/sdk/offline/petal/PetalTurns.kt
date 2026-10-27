package org.hyperledger.iroha.sdk.offline.petal

import java.util.Arrays

/**
 * The turn layer: data in the way the lit petals point.
 *
 * A field lane tells the decoder exactly which petals of its sector are lit. The first
 * [TURN_CARRIERS] lit petals of the sector, in the fixed pseudo-random [carrierOrder], carry two
 * more bits each: the petal is turned about its own centre by 0°, 45°, 90° or 135° from its
 * outward direction ([turnAngle]). A turned petal stays inside its own cell, so it moves no light
 * toward its neighbours and the field lanes read as before; a camera that resolves the petals'
 * shapes also reads the sector's turn lane.
 *
 * The turn lane of a sector (`T0`–`T3`) is one Reed–Solomon codeword, RS(90, 68), XOR-whitened:
 * four fountain atoms from the repair range (ids from [TURN_ID_BASE], see [turnFirstId]) and a
 * CRC-32C check that binds them to the stream tag, frame and sector of the field lane they ride
 * on ([turnCheck]). Carrier `j` shows bits `2j` and `2j + 1` of the transmitted codeword (most
 * significant bit first) as a Gray-coded position ([turnPosition]). A sector with fewer than
 * [TURN_CARRIERS] lit petals turns none (about one sector in 60,000).
 *
 * [readTurns] reads the turn lanes of a decoded capture: every carrier's neighbourhood is sampled
 * in the petal's own outward frame, following a smooth correction of the pose fitted to the
 * carriers' centroids; the blur is the one under which the carriers and their lit neighbours
 * explain the image best; and each carrier takes the turn whose (slightly wider) template matches
 * best once its lit neighbours, at their own decided turns, are subtracted.
 *
 * Atom ids are raw unsigned 32-bit patterns. All functions are thread-safe; the reader's tables
 * are built once, on first use. [PetalScanSession] reads the turn lanes of every capture it
 * decodes, once per shown frame, with scratch buffers it reuses.
 */
object PetalTurns {
    /** Turn positions a carrier petal can take. */
    const val TURN_POSITIONS = 4

    /** Bits one carrier shows. */
    const val TURN_BITS = 2

    /** Carrier petals of a sector. */
    const val TURN_CARRIERS = 360

    /** Codeword length of a turn lane in bytes. */
    const val TURN_WORD = TURN_CARRIERS * TURN_BITS / 8

    /** Parity bytes of a turn lane. */
    const val TURN_PARITY = 22

    /** Data bytes of a turn lane: the atoms, then their check. */
    const val TURN_DATA = TURN_WORD - TURN_PARITY

    /** Atoms carried by each turn lane. */
    const val TURN_ATOMS = 4

    /** Bytes of the check that closes a turn lane's data. */
    const val TURN_CHECK_LEN = 4

    /**
     * Fountain id of the first turn atom of frame 0, `2³¹` as a raw unsigned 32-bit pattern. The
     * field lanes and lane `D` number their atoms from 0 and never come near it.
     */
    const val TURN_ID_BASE = Int.MIN_VALUE

    /** Gaussian blurs (σ in canvas units) the turn reader fits, sharpest first. */
    @JvmField
    val TURN_BLURS: List<Double> = java.util.Collections.unmodifiableList(listOf(1.0, 1.4, 1.9, 2.5, 3.2, 4.0))

    private const val SECTORS = PetalLayout.SECTORS
    private const val ATOM_LEN = PetalLanes.ATOM_LEN

    /** The carrier order of every sector, sector `s` at `[s · 840, (s + 1) · 840)`. */
    private val CARRIER_ORDERS = IntArray(SECTORS * PetalLanes.FIELD_BITS)

    /** The whitening sequence of every turn lane, sector by sector. */
    private val WHITENING = arrayOfNulls<ByteArray>(SECTORS)

    private val CODEC = PetalReedSolomon(TURN_PARITY)

    init {
        check(TURN_DATA == TURN_ATOMS * ATOM_LEN + TURN_CHECK_LEN)
        check(TURN_CARRIERS <= PetalLanes.FIELD_BITS)
        check(1 shl TURN_BITS == TURN_POSITIONS)
        check(TURN_CARRIERS * TURN_BITS % 8 == 0)
        // Plain loops instead of constructor lambdas, as in `PetalLayout`'s initializer.
        for (sector in 0 until SECTORS) {
            val rng = PetalXorshift32(0x5452_4E30 + sector) // "TRN0" … "TRN3"
            val whitening = ByteArray(TURN_WORD)
            for (index in 0 until TURN_WORD) whitening[index] = rng.nextByte().toByte()
            WHITENING[sector] = whitening
        }
        for (sector in 0 until SECTORS) {
            val base = sector * PetalLanes.FIELD_BITS
            System.arraycopy(PetalLayout.SECTOR_ORDER, sector * PetalLayout.SECTOR_CELLS, CARRIER_ORDERS, base, PetalLanes.FIELD_BITS)
            val rng = PetalXorshift32(0x4F52_4430 + sector) // "ORD0" … "ORD3"
            for (i in PetalLanes.FIELD_BITS - 1 downTo 1) {
                val j = Integer.remainderUnsigned(rng.nextInt(), i + 1)
                val swap = CARRIER_ORDERS[base + i]
                CARRIER_ORDERS[base + i] = CARRIER_ORDERS[base + j]
                CARRIER_ORDERS[base + j] = swap
            }
        }
    }

    /**
     * The angle (radians, clockwise on screen) by which a petal at turn position [position] is
     * turned from its outward direction: `π · position / 4`.
     */
    @JvmStatic
    fun turnAngle(position: Int): Double = Math.PI * position.toDouble() / TURN_POSITIONS.toDouble()

    /**
     * The two bits a turn position shows: its Gray code `position ^ (position >> 1)`. Neighbouring
     * positions (135° and 0° neighbour too, a petal turned by 180° differing only by its notch)
     * differ in one bit.
     */
    @JvmStatic
    fun turnLabel(position: Int): Int = position xor (position ushr 1)

    /** The turn position that shows the two bits [label] (the inverse of [turnLabel]). */
    @JvmStatic
    fun turnPosition(label: Int): Int = label xor (label ushr 1)

    /**
     * The order in which the lit petals of sector [sector] (`0..3`) become carriers: its codeword
     * cells (the first `8 ×` [PetalLanes.FIELD_WORD] of [PetalLayout.sectorCells]) shuffled by
     * Fisher–Yates with a [PetalXorshift32] seeded `0x4F524430 + sector` ("ORD0" … "ORD3"): for
     * `i` from 839 down to 1, position `i` swaps with position `nextInt() mod (i + 1)` (unsigned).
     */
    @JvmStatic
    fun carrierOrder(sector: Int): IntArray {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        return CARRIER_ORDERS.copyOfRange(sector * PetalLanes.FIELD_BITS, (sector + 1) * PetalLanes.FIELD_BITS)
    }

    /**
     * The carriers of sector [sector] under the lit map [petals] (one flag per field cell): the
     * first [TURN_CARRIERS] lit cells in [carrierOrder], or `null` when fewer are lit.
     */
    @JvmStatic
    fun carriers(petals: BooleanArray, sector: Int): IntArray? {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        require(petals.size >= PetalLayout.FIELD_CELLS) { "a lit map has ${PetalLayout.FIELD_CELLS} flags" }
        val chosen = IntArray(TURN_CARRIERS)
        return if (fillCarriers(petals, sector, chosen, 0)) chosen else null
    }

    /** Writes the carriers of [sector] under [petals] into `out[offset until offset + 360]`; `false` when fewer are lit. */
    internal fun fillCarriers(petals: BooleanArray, sector: Int, out: IntArray, offset: Int): Boolean {
        var count = 0
        val base = sector * PetalLanes.FIELD_BITS
        for (index in base until base + PetalLanes.FIELD_BITS) {
            val cell = CARRIER_ORDERS[index]
            if (!petals[cell]) continue
            out[offset + count] = cell
            count += 1
            if (count == TURN_CARRIERS) return true
        }
        return false
    }

    /** Fountain id of the first of the [TURN_ATOMS] atoms the turn lane of [sector] carries in [frame]: `2³¹ + 16 · frame + 4 · sector`. */
    @JvmStatic
    fun turnFirstId(frame: Int, sector: Int): Int {
        PetalStream.checkFrame(frame)
        require(sector in 0 until SECTORS) { "sector index out of range" }
        return TURN_ID_BASE + frame * (SECTORS * TURN_ATOMS) + sector * TURN_ATOMS
    }

    /**
     * The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the sector
     * and the [atoms], so a turn lane read under the wrong field lane, frame or stream is refused.
     */
    @JvmStatic
    fun turnCheck(header: PetalLaneHeader, sector: Int, atoms: ByteArray): Int {
        require(sector in 0..0xFF) { "sector must be a byte" }
        return checkOf(header, sector, atoms, 0, atoms.size, ByteArray(4 + atoms.size))
    }

    /** CRC-32C of `tag, frame:u16, sector, atoms[offset until offset + length]`, using [scratch] (at least `4 + length` bytes). */
    private fun checkOf(header: PetalLaneHeader, sector: Int, atoms: ByteArray, offset: Int, length: Int, scratch: ByteArray): Int {
        scratch[0] = header.tag.toByte()
        scratch[1] = (header.frame ushr 8).toByte()
        scratch[2] = header.frame.toByte()
        scratch[3] = sector.toByte()
        System.arraycopy(atoms, offset, scratch, 4, length)
        return PetalCrc32c.compute(scratch, 0, 4 + length)
    }

    /** The data of a turn lane: the [TURN_ATOMS] [atoms] followed by their [turnCheck] (big-endian). */
    @JvmStatic
    fun turnData(header: PetalLaneHeader, sector: Int, atoms: ByteArray): ByteArray {
        require(atoms.size == TURN_ATOMS * ATOM_LEN) { "turn atoms length" }
        val data = atoms.copyOf(TURN_DATA)
        val check = turnCheck(header, sector, atoms)
        for (index in 0 until TURN_CHECK_LEN) data[TURN_ATOMS * ATOM_LEN + index] = (check ushr (24 - 8 * index)).toByte()
        return data
    }

    /** The atoms of turn lane [data] whose check matches [header] and [sector]; `null` otherwise. */
    @JvmStatic
    fun turnPacket(header: PetalLaneHeader, sector: Int, data: ByteArray): PetalAtomPacket? {
        if (data.size != TURN_DATA || sector !in 0 until SECTORS) return null
        if (!checkMatches(header, sector, data, ByteArray(TURN_DATA))) return null
        return PetalAtomPacket(header, turnFirstId(header.frame, sector), data.copyOf(TURN_ATOMS * ATOM_LEN))
    }

    /** Whether the check closing turn lane [data] matches [header] and [sector]. */
    private fun checkMatches(header: PetalLaneHeader, sector: Int, data: ByteArray, scratch: ByteArray): Boolean {
        val check = checkOf(header, sector, data, 0, TURN_ATOMS * ATOM_LEN, scratch)
        for (index in 0 until TURN_CHECK_LEN) {
            if (data[TURN_ATOMS * ATOM_LEN + index] != (check ushr (24 - 8 * index)).toByte()) return false
        }
        return true
    }

    /**
     * The whitening sequence of the turn lane of [sector]: one byte per codeword byte from a
     * [PetalXorshift32] seeded `0x54524E30 + sector` ("TRN0" … "TRN3").
     */
    @JvmStatic
    fun turnWhitening(sector: Int): ByteArray {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        return WHITENING[sector]!!.copyOf()
    }

    /** Encodes turn lane [data] (exactly [TURN_DATA] bytes) into the transmitted (whitened) codeword. */
    @JvmStatic
    fun encodeTurnLane(sector: Int, data: ByteArray): ByteArray {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        require(data.size == TURN_DATA) { "turn data length mismatch" }
        val word = CODEC.encode(data)
        val whitening = WHITENING[sector]!!
        for (index in word.indices) word[index] = (word[index].toInt() xor whitening[index].toInt()).toByte()
        return word
    }

    /**
     * Decodes a transmitted turn codeword, returning its data with how many byte positions the
     * Reed–Solomon decoder rewrote ([PetalLaneResult.corrected]). [erasures] lists byte positions
     * the caller distrusts. The check is not verified here (see [turnPacket]).
     *
     * @throws PetalReedSolomonException when the word is malformed or uncorrectable.
     */
    @JvmStatic
    @JvmOverloads
    fun decodeTurnLaneCounted(sector: Int, transmitted: ByteArray, erasures: IntArray = IntArray(0)): PetalLaneResult {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        return decodeOrNull(sector, transmitted, erasures, erasures.size)
            ?: throw PetalReedSolomonException(
                if (transmitted.size != TURN_WORD || !validErasures(erasures)) {
                    PetalReedSolomonException.Reason.INVALID_SHAPE
                } else {
                    PetalReedSolomonException.Reason.UNCORRECTABLE
                },
            )
    }

    private fun validErasures(erasures: IntArray): Boolean {
        if (erasures.size > TURN_PARITY) return false
        val seen = BooleanArray(TURN_WORD)
        for (position in erasures) {
            if (position < 0 || position >= TURN_WORD || seen[position]) return false
            seen[position] = true
        }
        return true
    }

    /** [decodeTurnLaneCounted] without exceptions; uses the first [erasureCount] erasures. */
    internal fun decodeOrNull(sector: Int, transmitted: ByteArray, erasures: IntArray, erasureCount: Int): PetalLaneResult? {
        if (transmitted.size != TURN_WORD) return null
        val whitening = WHITENING[sector]!!
        val word = ByteArray(TURN_WORD) { (transmitted[it].toInt() xor whitening[it].toInt()).toByte() }
        val corrected = CODEC.decodeInPlace(word, erasures, erasureCount)
        if (corrected < 0) return null
        return PetalLaneResult(word.copyOf(TURN_DATA), corrected, erasureCount)
    }

    /** The turn position carrier [index] (`0..359`) of the transmitted turn codeword [word] shows. */
    @JvmStatic
    fun carrierPosition(word: ByteArray, index: Int): Int {
        require(index in 0 until TURN_CARRIERS && word.size >= TURN_WORD) { "carrier index out of range" }
        return turnPosition(((word[index / 4].toInt() and 0xFF) ushr (6 - 2 * (index % 4))) and 3)
    }

    /**
     * Reads the turn lanes of the sectors [frame] decoded from [image], except those flagged in
     * [skip] (four flags, sector 0 first).
     *
     * The carriers of every decoded sector are decided (so that each carrier's neighbours are
     * modelled at their own turns), but only the lanes of the sectors not skipped are decoded, and
     * none when the image is too blurred to show turns. A lane passes when its Reed–Solomon
     * codeword decodes, with 0, 2, 5, 7 or 11 of its least confident bytes erased (a byte's
     * confidence is the smallest lead of its four carriers' turns), into data whose [turnCheck]
     * matches its sector's field lane header. Returns the lane data (atoms and check) per sector:
     * four entries, `null` where no turn lane was read. Each call allocates its own scratch
     * buffers (about 2 MB); [PetalScanSession] reuses them.
     */
    @JvmStatic
    @JvmOverloads
    fun readTurns(
        image: PetalLuma,
        frame: PetalDecodedFrame,
        skip: BooleanArray = BooleanArray(SECTORS),
    ): List<PetalLaneResult?> {
        require(skip.size == SECTORS) { "skip has one flag per sector" }
        val results = arrayOfNulls<PetalLaneResult>(SECTORS)
        PetalTurnReader().read(image, frame, skip, results)
        return java.util.Collections.unmodifiableList(results.toList())
    }

    /**
     * What [readTurns] decides for every carrier of the sectors [frame] decoded, before any
     * Reed–Solomon correction and whatever the blur, for diagnostics; `null` when no decoded
     * sector has carriers.
     */
    @JvmStatic
    fun observedTurns(image: PetalLuma, frame: PetalDecodedFrame): PetalObservedTurns? =
        PetalTurnReader().observe(image, frame)

    /**
     * The turn reader's blurred petal template for blur [blur] (an index into [TURN_BLURS]) at a
     * point of the unturned petal's own frame ([along] its axis, [across] it): what conformance
     * tests compare an SDK's tables with.
     */
    @JvmStatic
    fun turnTemplate(blur: Int, along: Double, across: Double): Double {
        require(blur in TURN_BLURS.indices) { "blur index out of range" }
        return PetalTurnTables.at(blur * PetalTurnTables.TEMPLATE_CELLS, along, across)
    }

    /**
     * Tries Reed–Solomon with 0, 2, 5, 7 and 11 of the least confident bytes of [word] erased (a
     * stable ranking: ties keep byte order) and returns the first result whose check matches
     * [header] and [sector]. [order] and [trial] are scratch (at least [TURN_WORD] entries).
     */
    internal fun decodeTurnWord(
        sector: Int,
        header: PetalLaneHeader,
        word: ByteArray,
        confidence: DoubleArray,
        order: IntArray,
        trial: ByteArray,
        checkScratch: ByteArray,
    ): PetalLaneResult? {
        // stable sort of byte positions by confidence (`sort_by` with `total_cmp`)
        for (i in 0 until TURN_WORD) {
            val key = confidence[i]
            var j = i - 1
            while (j >= 0 && PetalNumerics.totalCompare(confidence[order[j]], key) > 0) {
                order[j + 1] = order[j]
                j -= 1
            }
            order[j + 1] = i
        }
        val nsym = TURN_PARITY
        val schedule = intArrayOf(0, nsym / 8, nsym / 4, nsym / 3, nsym / 2)
        var previous = -1
        for (erasures in schedule) {
            if (erasures == previous) continue // `dedup` of consecutive repeats
            previous = erasures
            System.arraycopy(word, 0, trial, 0, TURN_WORD)
            for (index in 0 until erasures) trial[order[index]] = 0
            val result = decodeOrNull(sector, trial, order, erasures) ?: continue
            if (checkMatches(header, sector, result.dataView, checkScratch)) return result
        }
        return null
    }
}

/** What the turn reader decided for one capture (see [PetalTurns.observedTurns]). */
class PetalObservedTurns internal constructor(
    /** The blur (Gaussian σ in canvas units) that fits the carriers best. */
    val blur: Double,
    /** Every carrier of the decoded sectors, sector by sector in carrier order. */
    val carriers: List<PetalObservedCarrier>,
)

/** One carrier petal as the turn reader saw it. */
class PetalObservedCarrier internal constructor(
    /** Field cell index (`0..3375`). */
    val cell: Int,
    /** The turn position (`0..3`) decided for it. */
    val position: Int,
    /** How far that position's score led the runner-up's. */
    val lead: Double,
) {
    override fun toString(): String = "PetalObservedCarrier(cell=$cell, position=$position, lead=$lead)"
}

/**
 * The turn reader's fixed tables, built once on first use (class initialisation is lazy and
 * thread-safe): the window grid and its weights, the six blurred petal templates, the weighted
 * kernel of every turn position under every blur, and the near neighbours of every field cell.
 */
internal object PetalTurnTables {
    /** Half the side, in canvas units, of the square a petal template is tabulated over. */
    const val TEMPLATE_HALF = 9.0

    /** Template table step in canvas units. */
    const val TEMPLATE_STEP = 0.25

    /** Template table side in steps. */
    const val TEMPLATE_SIDE = 73

    /** Cells of one template table. */
    const val TEMPLATE_CELLS = TEMPLATE_SIDE * TEMPLATE_SIDE

    /** The window of a carrier is sampled every canvas unit over ±[GRID_HALF] units in its own frame. */
    const val GRID_HALF = 6

    /** Window side in samples. */
    const val GRID_SIDE = 2 * GRID_HALF + 1

    /** Samples of one window. */
    const val POINTS = GRID_SIDE * GRID_SIDE

    /** σ in canvas units of the Gaussian that weights the window. */
    const val WINDOW_SIGMA = 3.6

    /** Lit petals within this distance of a carrier are modelled as its neighbours. */
    const val NEIGHBOUR_DISTANCE = 19.0

    /**
     * A neighbour's template is added or subtracted within this distance of its centre. Radially
     * adjacent cell centres are 11 units apart and the window grid is integral, so a whole number
     * here would put grid points exactly on the boundary, where the last bit of a platform's `sin`
     * and `cos` decides.
     */
    const val NEIGHBOUR_REACH = 9.5

    /** [PetalTurns.TURN_BLURS] as an array. */
    val BLURS = PetalTurns.TURN_BLURS.toDoubleArray()

    /** Sine and cosine of every turn angle. */
    val TURN_SIN = DoubleArray(PetalTurns.TURN_POSITIONS)
    val TURN_COS = DoubleArray(PetalTurns.TURN_POSITIONS)

    /** The window grid, along the outward direction fastest: `a`, `b` and the weight of every sample. */
    val GRID_A = DoubleArray(POINTS)
    val GRID_B = DoubleArray(POINTS)
    val GRID_W = DoubleArray(POINTS)

    /** The six templates back to back, [TEMPLATE_CELLS] each (row `j` across, column `i` along). */
    val TEMPLATES = DoubleArray(BLURS.size * TEMPLATE_CELLS)

    /**
     * The template of turn position `p` under blur `σ` on the window grid, turned by `π·p/4`, at
     * `[(σ · 4 + p) · POINTS, … + POINTS)`: the model of a carrier at its own turn.
     */
    val TURNED = DoubleArray(BLURS.size * PetalTurns.TURN_POSITIONS * POINTS)

    /** The kernel of turn position `p` under blur `σ`, the grid weights times [TURNED], at the same offsets. */
    val KERNELS = DoubleArray(BLURS.size * PetalTurns.TURN_POSITIONS * POINTS)

    /**
     * The cells within [NEIGHBOUR_DISTANCE] of every field cell (itself excluded), in index order,
     * at `NEAR_START[c] until NEAR_START[c + 1]`: the other cell, its centre in this cell's outward
     * frame (`along`, `across`) and its outward direction relative to this cell's.
     */
    val NEAR_START = IntArray(PetalLayout.FIELD_CELLS + 1)
    val NEAR_CELL: ShortArray
    val NEAR_ALONG: DoubleArray
    val NEAR_ACROSS: DoubleArray
    val NEAR_ANGLE: DoubleArray

    init {
        for (position in 0 until PetalTurns.TURN_POSITIONS) {
            val angle = PetalTurns.turnAngle(position)
            TURN_SIN[position] = StrictMath.sin(angle)
            TURN_COS[position] = StrictMath.cos(angle)
        }
        var at = 0
        for (b in -GRID_HALF..GRID_HALF) {
            for (a in -GRID_HALF..GRID_HALF) {
                val x = a.toDouble()
                val y = b.toDouble()
                GRID_A[at] = x
                GRID_B[at] = y
                GRID_W[at] = StrictMath.exp(-(x * x + y * y) / (2.0 * WINDOW_SIGMA * WINDOW_SIGMA))
                at += 1
            }
        }
        // The coverage of every table cell by the unturned petal (4 × 4 samples per cell); it is
        // the same for every blur. The samples of a cell lie within an eighth of a unit of its
        // centre, so a cell whose centre is that far outside the petal's bounding box is empty.
        val side = TEMPLATE_SIDE
        val cover = DoubleArray(TEMPLATE_CELLS)
        val reachAlong = PetalLayout.PETAL_HALF_LENGTH_EXACT + TEMPLATE_STEP / 2.0
        val reachAcross = PetalLayout.PETAL_HALF_WIDTH_EXACT + TEMPLATE_STEP / 2.0
        for (j in 0 until side) {
            for (i in 0 until side) {
                val along = -TEMPLATE_HALF + i.toDouble() * TEMPLATE_STEP
                val across = -TEMPLATE_HALF + j.toDouble() * TEMPLATE_STEP
                if (Math.abs(along) > reachAlong || Math.abs(across) > reachAcross) continue
                var hits = 0
                for (sj in 0 until 4) {
                    for (si in 0 until 4) {
                        val da = (si.toDouble() - 1.5) * TEMPLATE_STEP / 4.0
                        val dc = (sj.toDouble() - 1.5) * TEMPLATE_STEP / 4.0
                        if (PetalLayout.petalLit(along + da, across + dc, 0.0, 1.0)) hits += 1
                    }
                }
                cover[j * side + i] = hits.toDouble() / 16.0
            }
        }
        val rows = DoubleArray(TEMPLATE_CELLS)
        for ((index, sigma) in BLURS.withIndex()) {
            // blurred by a Gaussian of σ truncated at 3σ: rows, then columns; taps outside the
            // table count as zero, and the kernel is normalised by its full sum
            val radius = Math.ceil(3.0 * sigma / TEMPLATE_STEP).toInt()
            val taps = DoubleArray(2 * radius + 1)
            var norm = -0.0
            for (t in taps.indices) {
                val k = (t - radius).toDouble() * TEMPLATE_STEP
                taps[t] = StrictMath.exp(-(k * k) / (2.0 * sigma * sigma))
                norm += taps[t]
            }
            blur(cover, rows, taps, radius, norm, true)
            blur(rows, TEMPLATES, taps, radius, norm, false, index * TEMPLATE_CELLS)
        }
        for (blur in BLURS.indices) {
            val template = blur * TEMPLATE_CELLS
            for (position in 0 until PetalTurns.TURN_POSITIONS) {
                val sin = TURN_SIN[position]
                val cos = TURN_COS[position]
                val kernel = (blur * PetalTurns.TURN_POSITIONS + position) * POINTS
                for (g in 0 until POINTS) {
                    val a = GRID_A[g]
                    val b = GRID_B[g]
                    TURNED[kernel + g] = at(template, a * cos + b * sin, -a * sin + b * cos)
                    KERNELS[kernel + g] = GRID_W[g] * TURNED[kernel + g]
                }
            }
        }
        // Bands are 11 units apart, so only the two bands on either side can hold neighbours.
        var cells = ShortArray(PetalLayout.FIELD_CELLS * 8)
        var along = DoubleArray(cells.size)
        var across = DoubleArray(cells.size)
        var angles = DoubleArray(cells.size)
        var count = 0
        val limit = NEIGHBOUR_DISTANCE * NEIGHBOUR_DISTANCE
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            val x = PetalLayout.CELL_X[cell]
            val y = PetalLayout.CELL_Y[cell]
            val angle = PetalLayout.CELL_ANGLE[cell]
            val sin = PetalLayout.CELL_SIN[cell]
            val cos = PetalLayout.CELL_COS[cell]
            val band = PetalLayout.CELL_BAND[cell]
            val first = PetalLayout.BAND_OFFSETS[maxOf(band - 2, 0)]
            val last = PetalLayout.BAND_OFFSETS[minOf(band + 3, PetalLayout.BAND_COUNT)]
            NEAR_START[cell] = count
            for (other in first until last) {
                if (other == cell) continue
                val dx = PetalLayout.CELL_X[other] - x
                val dy = PetalLayout.CELL_Y[other] - y
                if (dx * dx + dy * dy > limit) continue
                if (count == cells.size) {
                    cells = cells.copyOf(2 * count)
                    along = along.copyOf(2 * count)
                    across = across.copyOf(2 * count)
                    angles = angles.copyOf(2 * count)
                }
                cells[count] = other.toShort()
                along[count] = dx * cos + dy * sin
                across[count] = -dx * sin + dy * cos
                angles[count] = PetalLayout.CELL_ANGLE[other] - angle
                count += 1
            }
        }
        NEAR_START[PetalLayout.FIELD_CELLS] = count
        NEAR_CELL = cells.copyOf(count)
        NEAR_ALONG = along.copyOf(count)
        NEAR_ACROSS = across.copyOf(count)
        NEAR_ANGLE = angles.copyOf(count)
    }

    /**
     * One blur pass of a template table: along rows (`i`) or along columns (`j`), into
     * `out[outOffset …]`. Taps run in order over the cells inside the table (the others count as
     * zero); a row of zeros blurs along itself to zeros.
     */
    private fun blur(
        source: DoubleArray,
        out: DoubleArray,
        taps: DoubleArray,
        radius: Int,
        norm: Double,
        rows: Boolean,
        outOffset: Int = 0,
    ) {
        val side = TEMPLATE_SIDE
        for (j in 0 until side) {
            if (rows) {
                var empty = true
                for (i in 0 until side) if (source[j * side + i] != 0.0) empty = false
                if (empty) {
                    Arrays.fill(out, outOffset + j * side, outOffset + (j + 1) * side, 0.0)
                    continue
                }
            }
            for (i in 0 until side) {
                val centre = if (rows) i else j
                val first = maxOf(0, radius - centre)
                val last = minOf(taps.size - 1, radius + side - 1 - centre)
                val step = if (rows) 1 else side
                var at = j * side + i + (first - radius) * step
                var sum = 0.0
                for (t in first..last) {
                    sum += taps[t] * source[at]
                    at += step
                }
                out[outOffset + j * side + i] = sum / norm
            }
        }
    }

    /** The template at `TEMPLATES[template …]` at a point of the petal's own frame (bilinear; zero outside the table). */
    fun at(template: Int, along: Double, across: Double): Double {
        val x = (along + TEMPLATE_HALF) / TEMPLATE_STEP
        val y = (across + TEMPLATE_HALF) / TEMPLATE_STEP
        val last = (TEMPLATE_SIDE - 1).toDouble()
        if (!(x >= 0.0 && y >= 0.0 && x < last && y < last)) return 0.0
        val i = x.toInt()
        val j = y.toInt()
        val fx = x - i.toDouble()
        val fy = y - j.toDouble()
        val t = TEMPLATES
        val row = template + j * TEMPLATE_SIDE + i
        val top = t[row] * (1.0 - fx) + t[row + 1] * fx
        val bottom = t[row + TEMPLATE_SIDE] * (1.0 - fx) + t[row + TEMPLATE_SIDE + 1] * fx
        return top * (1.0 - fy) + bottom * fy
    }
}

/**
 * The turn reader with its scratch buffers (`decide_turns` and `read_turns` of the reference):
 * the windows of up to 1,440 carriers (1.9 MB) and a few small arrays, reused between captures.
 * Not thread-safe: one reader per decoding thread (a [PetalWorkspace] holds one).
 */
internal class PetalTurnReader {
    private companion object {
        const val SECTORS = PetalLayout.SECTORS
        const val CARRIERS = PetalTurns.TURN_CARRIERS
        const val POINTS = PetalTurnTables.POINTS
        const val SIDE = PetalTurnTables.GRID_SIDE
        const val HALF = PetalTurnTables.GRID_HALF

        /** From this fitted blur on (3.2 units), the camera cannot resolve the turns and the reader stops. */
        const val BLUR_LIMIT = 4

        /** σ in canvas units of the Gaussian that follows a carrier's centroid. */
        const val CENTROID_SIGMA = 2.6

        /** The warp and the blur are fitted on every this-many-th carrier. */
        const val SAMPLE_STEP = 17

        /** The 17th smallest of a window's 169 samples (its 10th percentile) becomes 0 … */
        const val LOW_RANK = POINTS / 10

        /** … and its 166th smallest (its 98th percentile) becomes 1. */
        const val HIGH_RANK = POINTS * 98 / 100

        /** Bins of the sample counts of [normalise]: one per grey level. */
        const val BINS = 256
    }

    private val lit = BooleanArray(PetalLayout.FIELD_CELLS)
    private val headers = arrayOfNulls<PetalLaneHeader>(SECTORS)
    private val rangeStart = IntArray(SECTORS)
    private val carrierCells = IntArray(SECTORS * CARRIERS)
    private var carrierCount = 0
    private val carrierOf = IntArray(PetalLayout.FIELD_CELLS)
    private var windows = DoubleArray(0)

    /** Whether the window of a carrier has been sampled (windows are sampled when first needed). */
    private val sampled = BooleanArray(SECTORS * CARRIERS)
    private var image: PetalLuma? = null
    private var pose: PetalPose? = null
    private val scratch = DoubleArray(POINTS)
    private val positions = IntArray(SECTORS * CARRIERS)
    private val leads = DoubleArray(SECTORS * CARRIERS)
    private val blurPositions = IntArray(SECTORS * CARRIERS)
    private var fitted = 0

    /** The lead of the last [decide]. */
    internal var lead = 0.0
        private set

    // normalise
    private val bins = IntArray(BINS)
    private val lowMembers = DoubleArray(POINTS)
    private val highMembers = DoubleArray(POINTS)

    // pose, centroid and warp
    private val point = DoubleArray(6)
    private val alongWeights = DoubleArray(SIDE)
    private val acrossWeights = DoubleArray(SIDE)
    private var centroidA = 0.0
    private var centroidB = 0.0
    private val sampledX = DoubleArray((SECTORS * CARRIERS + SAMPLE_STEP - 1) / SAMPLE_STEP)
    private val sampledY = DoubleArray(sampledX.size)
    private val offsetsX = DoubleArray(sampledX.size)
    private val offsetsY = DoubleArray(sampledX.size)
    private val monomials = DoubleArray(10)
    private val normal = Array(10) { DoubleArray(11) }
    private val fitX = DoubleArray(10)
    private val fitY = DoubleArray(10)

    // codewords
    private val word = ByteArray(PetalTurns.TURN_WORD)
    private val confidence = DoubleArray(PetalTurns.TURN_WORD)
    private val order = IntArray(PetalTurns.TURN_WORD)
    private val trial = ByteArray(PetalTurns.TURN_WORD)
    private val checkScratch = ByteArray(PetalTurns.TURN_DATA)

    /** [PetalTurns.readTurns] into [results] (four entries, cleared first). */
    fun read(image: PetalLuma, frame: PetalDecodedFrame, skip: BooleanArray, results: Array<PetalLaneResult?>) {
        Arrays.fill(results, null)
        var need = 0
        for (sector in 0 until SECTORS) if (!skip[sector]) need = need or (1 shl sector)
        if (!decideTurns(image, frame, need, true)) return
        for (sector in 0 until SECTORS) {
            val start = rangeStart[sector]
            val header = headers[sector]
            if (start < 0 || header == null || skip[sector]) continue
            Arrays.fill(word, 0.toByte())
            Arrays.fill(confidence, Double.MAX_VALUE)
            for (j in 0 until CARRIERS) {
                val label = PetalTurns.turnLabel(positions[start + j])
                word[j / 4] = (word[j / 4].toInt() or (label shl (6 - 2 * (j % 4)))).toByte()
                confidence[j / 4] = PetalNumerics.min(confidence[j / 4], leads[start + j])
            }
            results[sector] = PetalTurns.decodeTurnWord(sector, header, word, confidence, order, trial, checkScratch)
        }
    }

    /** [PetalTurns.observedTurns]. */
    fun observe(image: PetalLuma, frame: PetalDecodedFrame): PetalObservedTurns? {
        if (!decideTurns(image, frame, (1 shl SECTORS) - 1, false)) return null
        val carriers = ArrayList<PetalObservedCarrier>(carrierCount)
        for (index in 0 until carrierCount) carriers += PetalObservedCarrier(carrierCells[index], positions[index], leads[index])
        return PetalObservedTurns(PetalTurnTables.BLURS[fitted], java.util.Collections.unmodifiableList(carriers))
    }

    /**
     * Decides the turn of every carrier of the sectors [frame] decoded, when a sector whose bit is
     * set in [need] has carriers; with [limit], not when the fitted blur reaches [BLUR_LIMIT].
     * Returns whether it decided: the turns are then in [positions] and [leads], sector by sector
     * from [rangeStart], and the fitted blur in [fitted].
     *
     * 1. The lit petals of the decoded sectors come from their field lanes, and the carriers of
     *    each from [PetalTurns.carriers].
     * 2. The pose's smooth error: for every carrier in steps of [SAMPLE_STEP], the window is
     *    sampled at its cell centre and normalised, and its centroid turned into a canvas offset;
     *    a cubic fitted to the offsets over the canvas moves every carrier's window, which is then
     *    sampled and normalised.
     * 3. The blur: for every carrier in steps of [SAMPLE_STEP], the model of the petal at its best
     *    turn plus its lit neighbours at theirs (each decided on its own window, unturned when it
     *    carries nothing) is correlated with the window, weighted by the window Gaussian; the blur
     *    with the largest sum fits best.
     * 4. The turns: with the next wider blur's kernels, every carrier takes the best turn of its
     *    own window, then, carrier by carrier in order, the best turn of its window with its lit
     *    neighbours' templates at their latest turns subtracted.
     */
    private fun decideTurns(image: PetalLuma, frame: PetalDecodedFrame, need: Int, limit: Boolean): Boolean {
        if (image.width <= 0 || image.height <= 0) return false
        // the lit petals of every sector that decoded
        Arrays.fill(lit, false)
        Arrays.fill(headers, null)
        for (sector in 0 until SECTORS) {
            val data = frame.fields[sector]?.dataView ?: continue
            if (data.size != PetalLanes.FIELD_DATA) continue
            headers[sector] = PetalLaneHeader(data[0].toInt() and 0xFF, ((data[1].toInt() and 0xFF) shl 8) or (data[2].toInt() and 0xFF))
            val word = PetalLanes.encodeLane(PetalLane.field(sector), data)
            val base = sector * PetalLayout.SECTOR_CELLS
            for (bit in 0 until PetalLanes.FIELD_BITS) {
                lit[PetalLayout.SECTOR_ORDER[base + bit]] = ((word[bit / 8].toInt() ushr (7 - bit % 8)) and 1) == 1
            }
        }
        // the carriers of every decoded sector, sector by sector in carrier order
        carrierCount = 0
        var needed = false
        for (sector in 0 until SECTORS) {
            rangeStart[sector] = -1
            if (headers[sector] == null) continue
            if (PetalTurns.fillCarriers(lit, sector, carrierCells, carrierCount)) {
                rangeStart[sector] = carrierCount
                carrierCount += CARRIERS
                if (need and (1 shl sector) != 0) needed = true
            }
        }
        if (!needed) return false
        val count = carrierCount
        if (windows.size < count * POINTS) windows = DoubleArray(SECTORS * CARRIERS * POINTS)
        val windows = windows
        Arrays.fill(carrierOf, -1)
        for (index in 0 until count) carrierOf[carrierCells[index]] = index
        try {
            this.image = image
            this.pose = frame.pose
            return decideCarriers(image, frame.pose, count, windows, limit)
        } finally {
            // the reader outlives the capture: let go of it
            this.image = null
            this.pose = null
        }
    }

    /** Steps 2 to 4 of [decideTurns] for its [count] carriers. */
    private fun decideCarriers(image: PetalLuma, pose: PetalPose, count: Int, windows: DoubleArray, limit: Boolean): Boolean {
        // the pose's smooth error, from the centroids of a sample of the carriers
        var sampled = 0
        for (index in 0 until count step SAMPLE_STEP) {
            val cell = carrierCells[index]
            sampleWindow(image, pose, cell, 0.0, 0.0, scratch, 0)
            normalise(scratch, 0)
            centroid(scratch)
            val sin = PetalLayout.CELL_SIN[cell]
            val cos = PetalLayout.CELL_COS[cell]
            sampledX[sampled] = PetalLayout.CELL_X[cell]
            sampledY[sampled] = PetalLayout.CELL_Y[cell]
            offsetsX[sampled] = centroidA * cos - centroidB * sin
            offsetsY[sampled] = centroidA * sin + centroidB * cos
            sampled += 1
        }
        fitCubic(sampled, offsetsX, fitX)
        fitCubic(sampled, offsetsY, fitY)
        // every carrier's window follows the fitted correction; it is sampled when first needed,
        // so a capture too blurred for turns costs the windows of the blur fit only
        Arrays.fill(this.sampled, 0, count, false)
        // the blur under which a sample of the carriers, each at its best turn with its lit
        // neighbours at theirs, explains the windows best
        val model = scratch
        var fitted = 0
        var bestScore = Double.NEGATIVE_INFINITY
        for (blur in PetalTurnTables.BLURS.indices) {
            val template = blur * PetalTurnTables.TEMPLATE_CELLS
            // a carrier's best turn under this blur, decided on its own window when first needed
            Arrays.fill(blurPositions, 0, count, -1)
            var score = 0.0
            for (i in 0 until count step SAMPLE_STEP) {
                val turn = turnOf(i, blur)
                System.arraycopy(PetalTurnTables.TURNED, (blur * PetalTurns.TURN_POSITIONS + turn) * POINTS, model, 0, POINTS)
                val cell = carrierCells[i]
                for (near in PetalTurnTables.NEAR_START[cell] until PetalTurnTables.NEAR_START[cell + 1]) {
                    val other = PetalTurnTables.NEAR_CELL[near].toInt()
                    if (!lit[other]) continue
                    val j = carrierOf[other]
                    val angle = if (j < 0) 0.0 else PetalTurns.turnAngle(turnOf(j, blur))
                    addNeighbour(model, 0, template, near, angle, 1.0)
                }
                var cross = 0.0
                var mm = 0.0
                var xx = 0.0
                val offset = window(i)
                for (g in 0 until POINTS) {
                    val w = PetalTurnTables.GRID_W[g]
                    val x = windows[offset + g]
                    val m = model[g]
                    cross += w * x * m
                    mm += w * m * m
                    xx += w * x * x
                }
                score += cross / PetalNumerics.max(Math.sqrt(mm * xx), 1e-9)
            }
            if (score > bestScore) {
                bestScore = score
                fitted = blur
            }
        }
        this.fitted = fitted
        if (limit && fitted >= BLUR_LIMIT) return false
        // the turns, decided with the next wider blur
        val blur = minOf(fitted + 1, PetalTurnTables.BLURS.size - 1)
        val template = blur * PetalTurnTables.TEMPLATE_CELLS
        for (index in 0 until count) {
            positions[index] = decide(windows, window(index), blur)
            leads[index] = lead
        }
        for (index in 0 until count) {
            System.arraycopy(windows, index * POINTS, scratch, 0, POINTS)
            val cell = carrierCells[index]
            for (near in PetalTurnTables.NEAR_START[cell] until PetalTurnTables.NEAR_START[cell + 1]) {
                val other = PetalTurnTables.NEAR_CELL[near].toInt()
                if (!lit[other]) continue
                val j = carrierOf[other]
                val angle = if (j < 0) 0.0 else PetalTurns.turnAngle(positions[j])
                addNeighbour(scratch, 0, template, near, angle, -1.0)
            }
            positions[index] = decide(scratch, 0, blur)
            leads[index] = lead
        }
        return true
    }

    /** The offset of carrier [index]'s window in [windows], sampled and normalised when first needed. */
    private fun window(index: Int): Int {
        val offset = index * POINTS
        if (!sampled[index]) {
            val cell = carrierCells[index]
            val x = PetalLayout.CELL_X[cell]
            val y = PetalLayout.CELL_Y[cell]
            sampleWindow(image!!, pose!!, cell, evalCubic(fitX, x, y), evalCubic(fitY, x, y), windows, offset)
            normalise(windows, offset)
            sampled[index] = true
        }
        return offset
    }

    /** The best turn of carrier [index]'s own window under blur [blur] (memoised per blur in [blurPositions]). */
    private fun turnOf(index: Int, blur: Int): Int {
        val known = blurPositions[index]
        if (known >= 0) return known
        val position = decide(windows, window(index), blur)
        blurPositions[index] = position
        return position
    }

    /**
     * The best turn position for the window at `window[offset …]` under blur [blur] (scores
     * `Σ window · kernel` in grid order; ties: the lower position), with how far its score leads
     * the runner-up's in [lead].
     */
    internal fun decide(window: DoubleArray, offset: Int, blur: Int): Int {
        val kernels = PetalTurnTables.KERNELS
        val k0 = blur * PetalTurns.TURN_POSITIONS * POINTS
        val k1 = k0 + POINTS
        val k2 = k1 + POINTS
        val k3 = k2 + POINTS
        // iterator sums start from -0.0
        var s0 = -0.0
        var s1 = -0.0
        var s2 = -0.0
        var s3 = -0.0
        for (g in 0 until POINTS) {
            val x = window[offset + g]
            s0 += x * kernels[k0 + g]
            s1 += x * kernels[k1 + g]
            s2 += x * kernels[k2 + g]
            s3 += x * kernels[k3 + g]
        }
        var best = 0
        var bestScore = s0
        if (s1 > bestScore) {
            best = 1
            bestScore = s1
        }
        if (s2 > bestScore) {
            best = 2
            bestScore = s2
        }
        if (s3 > bestScore) {
            best = 3
            bestScore = s3
        }
        var runnerUp = Double.NEGATIVE_INFINITY
        if (best != 0) runnerUp = PetalNumerics.max(runnerUp, s0)
        if (best != 1) runnerUp = PetalNumerics.max(runnerUp, s1)
        if (best != 2) runnerUp = PetalNumerics.max(runnerUp, s2)
        if (best != 3) runnerUp = PetalNumerics.max(runnerUp, s3)
        lead = bestScore - runnerUp
        return best
    }

    /**
     * Adds [sign] × the template of the neighbour at entry [near] of the neighbour table, turned
     * by [turn] radians, to the window at `window[offset …]`, at the grid points within
     * [PetalTurnTables.NEIGHBOUR_REACH] of the neighbour's centre.
     */
    private fun addNeighbour(window: DoubleArray, offset: Int, template: Int, near: Int, turn: Double, sign: Double) {
        val angle = PetalTurnTables.NEAR_ANGLE[near] + turn
        val sin = StrictMath.sin(angle)
        val cos = StrictMath.cos(angle)
        val along = PetalTurnTables.NEAR_ALONG[near]
        val across = PetalTurnTables.NEAR_ACROSS[near]
        val reach = PetalTurnTables.NEIGHBOUR_REACH
        val reach2 = reach * reach
        val half = HALF.toDouble()
        // only the grid rows and columns within reach of the neighbour's centre
        val a0 = PetalNumerics.max(Math.ceil(along - reach), -half).toInt()
        val a1 = PetalNumerics.min(Math.floor(along + reach), half).toInt()
        val b0 = PetalNumerics.max(Math.ceil(across - reach), -half).toInt()
        val b1 = PetalNumerics.min(Math.floor(across + reach), half).toInt()
        for (b in b0..b1) {
            val ub = b.toDouble() - across
            val ub2 = ub * ub
            val ubSin = ub * sin
            val ubCos = ub * cos
            val row = offset + (b + HALF) * SIDE + HALF
            for (a in a0..a1) {
                val ua = a.toDouble() - along
                if (ua * ua + ub2 > reach2) continue
                window[row + a] += sign * PetalTurnTables.at(template, ua * cos + ubSin, -ua * sin + ubCos)
            }
        }
    }

    /**
     * Samples [image] on the window grid of the petal at [cell], centred `(shiftX, shiftY)` canvas
     * units off the cell centre, in the petal's outward frame (grid rows across, columns along),
     * into `out[offset …]`. Within a window the pose is taken as affine: with `o` the image point
     * of the centre and `u`, `v` the image steps of one unit along and across, grid point `(a, b)`
     * is sampled at `(o + b·v) + a·u`.
     */
    private fun sampleWindow(
        image: PetalLuma,
        pose: PetalPose,
        cell: Int,
        shiftX: Double,
        shiftY: Double,
        out: DoubleArray,
        offset: Int,
    ) {
        val x = PetalLayout.CELL_X[cell] + shiftX
        val y = PetalLayout.CELL_Y[cell] + shiftY
        val sin = PetalLayout.CELL_SIN[cell]
        val cos = PetalLayout.CELL_COS[cell]
        val p = point
        pose.applyInto(x, y, p, 0)
        pose.applyInto(x + cos, y + sin, p, 2)
        pose.applyInto(x - sin, y + cos, p, 4)
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
        var index = offset
        for (bi in -HALF..HALF) {
            val b = bi.toDouble()
            val rowX = p[0] + b * vx
            val rowY = p[1] + b * vy
            for (ai in -HALF..HALF) {
                val a = ai.toDouble()
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
                out[index] = top * (1.0 - ty) + bottom * ty
                index += 1
            }
        }
    }

    /**
     * Rescales the window at `window[offset …]` so that its 10th percentile (of 169 samples, the
     * 17th smallest) becomes 0 and its 98th (the 166th smallest) 1 (a span below one grey level
     * counts as one), clamped to −0.2 … 1.2.
     */
    private fun normalise(window: DoubleArray, offset: Int) {
        // Image samples lie in 0 … 255: count them by their integer part, find the bins that hold
        // the two ranks, and rank only the samples of those bins. With no NaN and no -0.0 the
        // numeric order is `total_cmp`'s.
        val bins = bins
        Arrays.fill(bins, 0)
        var plain = true
        for (g in offset until offset + POINTS) {
            val value = window[g]
            if (!(value >= 0.0 && value < BINS) || (value == 0.0 && java.lang.Double.doubleToRawLongBits(value) != 0L)) {
                plain = false
                break
            }
            bins[value.toInt()] += 1
        }
        val bottom: Double
        val top: Double
        if (plain) {
            var below = 0
            var lowBin = 0
            while (below + bins[lowBin] <= LOW_RANK) {
                below += bins[lowBin]
                lowBin += 1
            }
            val lowIndex = LOW_RANK - below
            var above = 0
            var highBin = BINS - 1
            while (above + bins[highBin] <= POINTS - 1 - HIGH_RANK) {
                above += bins[highBin]
                highBin -= 1
            }
            val highIndex = bins[highBin] - 1 - (POINTS - 1 - HIGH_RANK - above)
            var lows = 0
            var highs = 0
            for (g in offset until offset + POINTS) {
                val value = window[g]
                val bin = value.toInt()
                if (bin == lowBin) {
                    lowMembers[lows] = value
                    lows += 1
                }
                if (bin == highBin) {
                    highMembers[highs] = value
                    highs += 1
                }
            }
            bottom = select(lowMembers, lows, lowIndex)
            top = select(highMembers, highs, highIndex)
        } else {
            // a broken pose: rank the samples by `total_cmp` the slow way
            val sorted = window.copyOfRange(offset, offset + POINTS)
            for (i in 1 until POINTS) {
                val value = sorted[i]
                var j = i - 1
                while (j >= 0 && PetalNumerics.totalCompare(sorted[j], value) > 0) {
                    sorted[j + 1] = sorted[j]
                    j -= 1
                }
                sorted[j + 1] = value
            }
            bottom = sorted[LOW_RANK]
            top = sorted[HIGH_RANK]
        }
        val span = PetalNumerics.max(top - bottom, 1.0)
        for (g in offset until offset + POINTS) {
            var value = (window[g] - bottom) / span
            if (value < -0.2) value = -0.2
            if (value > 1.2) value = 1.2
            window[g] = value
        }
    }

    /** The [rank]-th smallest (from 0) of the first [count] [values], which it sorts (insertion sort; few values). */
    private fun select(values: DoubleArray, count: Int, rank: Int): Double {
        for (i in 1 until count) {
            val value = values[i]
            var j = i - 1
            while (j >= 0 && values[j] > value) {
                values[j + 1] = values[j]
                j -= 1
            }
            values[j + 1] = value
        }
        return values[rank]
    }

    /**
     * The centroid (along, across) of the light of a normalised window into [centroidA] and
     * [centroidB]: every sample weighs its level (clamped to 0 … 1) times a Gaussian of σ
     * [CENTROID_SIGMA] that starts at the window centre and follows the centroid three times; the
     * Gaussian is its along factor times its across factor.
     */
    private fun centroid(window: DoubleArray) {
        var ea = 0.0
        var eb = 0.0
        val k = 2.0 * CENTROID_SIGMA * CENTROID_SIGMA
        for (round in 0 until 3) {
            for (i in 0 until SIDE) {
                val t = (i - HALF).toDouble()
                alongWeights[i] = StrictMath.exp(-(t - ea) * (t - ea) / k)
                acrossWeights[i] = StrictMath.exp(-(t - eb) * (t - eb) / k)
            }
            var sum = 0.0
            var sa = 0.0
            var sb = 0.0
            for (j in 0 until SIDE) {
                val b = (j - HALF).toDouble()
                for (i in 0 until SIDE) {
                    val a = (i - HALF).toDouble()
                    var level = window[j * SIDE + i]
                    if (level < 0.0) level = 0.0
                    if (level > 1.0) level = 1.0
                    val w = level * alongWeights[i] * acrossWeights[j]
                    sum += w
                    sa += w * a
                    sb += w * b
                }
            }
            if (sum <= 0.0) break
            ea = sa / sum
            eb = sb / sum
        }
        centroidA = ea
        centroidB = eb
    }

    /** The ten cubic monomials of the canvas point `(x, y)`, centred on the canvas and scaled by 384, into [monomials]. */
    private fun monomials(x: Double, y: Double) {
        val u = (x - 512.0) / 384.0
        val v = (y - 512.0) / 384.0
        val f = monomials
        f[0] = 1.0
        f[1] = u
        f[2] = v
        f[3] = u * u
        f[4] = u * v
        f[5] = v * v
        f[6] = u * u * u
        f[7] = u * u * v
        f[8] = u * v * v
        f[9] = v * v * v
    }

    /**
     * The least-squares cubic through the first [count] [values] at the sampled carrier centres
     * into [out]: normal equations (summed point by point) with 10⁻⁶ added to the diagonal, solved
     * by Gauss–Jordan elimination with partial pivoting (a pivot below 10⁻¹² skips its column).
     */
    private fun fitCubic(count: Int, values: DoubleArray, out: DoubleArray) {
        val a = normal
        for (row in a) Arrays.fill(row, 0.0)
        val f = monomials
        for (p in 0 until count) {
            monomials(sampledX[p], sampledY[p])
            val value = values[p]
            for (r in 0 until 10) {
                val row = a[r]
                for (c in 0 until 10) row[c] += f[r] * f[c]
                row[10] += f[r] * value
            }
        }
        for (r in 0 until 10) a[r][r] += 1e-6
        for (col in 0 until 10) {
            var pivot = col
            for (r in col + 1 until 10) if (Math.abs(a[r][col]) > Math.abs(a[pivot][col])) pivot = r
            val swap = a[col]
            a[col] = a[pivot]
            a[pivot] = swap
            val pivotRow = a[col]
            val p = pivotRow[col]
            if (Math.abs(p) < 1e-12) continue
            for (c in col..10) pivotRow[c] /= p
            for (r in 0 until 10) {
                if (r == col) continue
                val row = a[r]
                val factor = row[col]
                for (c in col..10) row[c] -= factor * pivotRow[c]
            }
        }
        for (r in 0 until 10) out[r] = a[r][10]
    }

    private fun evalCubic(coefficients: DoubleArray, x: Double, y: Double): Double {
        monomials(x, y)
        var sum = -0.0
        for (index in 0 until 10) sum += monomials[index] * coefficients[index]
        return sum
    }
}
