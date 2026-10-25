package org.hyperledger.iroha.sdk.offline.petal

/** What a ring slot is used for. */
enum class PetalSlotRole {
    /** A gate dot, always lit. */
    GATE,

    /** A slot next to a gate, always dark. */
    GUARD,

    /** Carries one bit of lane `D` (see [PetalLayout.slotDataBit]). */
    DATA,

    /** Unused data slot, always dark. */
    SPARE,
}

/**
 * Normative Petal frame geometry (format 3).
 *
 * All coordinates are *design units* on a square canvas of [CANVAS] units with
 * the origin at the top-left and `y` growing downward. A renderer scales the
 * canvas to any pixel size; a decoder maps camera pixels back to design units.
 * Cell *order* is defined by integer indices only, and the slot count of every
 * band is a fixed table.
 */
object PetalLayout {
    /** Canvas side length in design units. */
    const val CANVAS = 1024.0

    /** Canvas centre coordinate on both axes. */
    const val CENTER = 512.0

    /** Number of petal bands (concentric rings of field cells). */
    const val BAND_COUNT = 34

    /** Radius of band 0. */
    const val BAND_INNER = 16.0

    /** Radial distance between consecutive bands. */
    const val BAND_SPACING = 11.0

    /** Number of field cells. */
    const val FIELD_CELLS = 3376

    /** Angular sectors of the field; each is one lane codeword. */
    const val SECTORS = 4

    /** Cells in each sector. */
    const val SECTOR_CELLS = FIELD_CELLS / SECTORS

    /**
     * Radius of the disc a petal stands in for: decoders sample within it, and no other petal,
     * however it is turned, reaches into it.
     */
    const val PETAL_RADIUS = 4.6

    /**
     * Half the length of a drawn petal, along its axis (the outward radial direction, turned by the
     * petal's turn, see [PetalTurns]).
     */
    const val PETAL_HALF_LENGTH = 6.0

    /** Half the width of a drawn petal. */
    const val PETAL_HALF_WIDTH = 3.73

    /** Radius of the notch cut into each petal tip; it tells a petal from one turned by half a turn. */
    const val PETAL_NOTCH_RADIUS = 1.12

    /** Distance from a petal centre to its notch centre, along its axis. */
    const val PETAL_NOTCH_DISTANCE = 6.24

    /** Number of lattice columns and rows of the `天` mask. */
    const val MASK_GRID = 20

    /** Canvas coordinate of the mask lattice's left and top edge. */
    const val MASK_ORIGIN = 182.0

    /** Mask lattice pitch in design units. */
    const val MASK_PITCH = 33.0

    /** Radius beyond which the `天` backdrop is not drawn. */
    const val BACKDROP_RADIUS = 390.0

    /** Number of concentric dot rings. */
    const val RING_COUNT = 3

    /** Radius of a drawn ring dot. */
    const val DOT_RADIUS = 9.0

    /** Total dot slots over the three rings. */
    const val TOTAL_SLOTS = 88 + 100 + 108

    /** Number of lane `D` bits carried by the rings. */
    const val D_BITS = 240

    /** Radius of the finder's solid centre disc. */
    const val FINDER_CORE = 12.0

    /** Number of petals of a finder blossom. */
    const val FINDER_PETALS = 5

    /** Distance from the finder centre to each petal centre. */
    const val FINDER_PETAL_DISTANCE = 34.0

    /** Radius of each petal. */
    const val FINDER_PETAL_RADIUS = 26.0

    /** Radius of the notch cut into each petal tip. */
    const val FINDER_NOTCH_RADIUS = 6.0

    /** Outer radius of a finder blossom (tip of a petal). */
    const val FINDER_OUTER = 60.0

    /**
     * The `天` silhouette, one string per lattice row from top to bottom. `#`
     * marks a lattice cell inside the character. Petals whose centre lies in a
     * `#` cell are drawn in the light colour, the others in pink, and the `#`
     * cells inside [BACKDROP_RADIUS] are drawn as a faint backdrop, so the
     * character shows through the petals. The mask is mirror-symmetric left to
     * right and not top to bottom.
     */
    @JvmField
    val MASK: List<String> = java.util.Collections.unmodifiableList(
        listOf(
            "....############....",
            "...##############...",
            "..################..",
            ".##################.",
            "###..............###",
            "###..............###",
            "#########..#########",
            "#########..#########",
            "###..............###",
            "###..............###",
            "########....########",
            "########....########",
            "#######......#######",
            "#######..##..#######",
            "######..####..######",
            "#####...####...#####",
            ".###...######...###.",
            "..###.########.###..",
            "......########......",
            ".....##########.....",
        ),
    )

    /**
     * Cells on each band, inner band first: `4 · round(2π r / 50)` for the band radius `r`, about
     * one cell every 12.5 units. The table, not the formula, is normative.
     */
    private val BAND_SLOTS = intArrayOf(
        8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124,
        128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
    )
    private val RING_RADII = doubleArrayOf(405.0, 440.0, 475.0)
    private val RING_RADII_F = floatArrayOf(405f, 440f, 475f)
    private val RING_SLOTS = intArrayOf(88, 100, 108)

    /** Dots per gate and ring for the right, bottom and left gates (no top gate). */
    private val GATE_DOTS = arrayOf(intArrayOf(1, 1, 3), intArrayOf(2, 2, 2), intArrayOf(1, 2, 2))
    private val FINDER_CENTERS = doubleArrayOf(72.0, 72.0, 952.0, 72.0, 952.0, 952.0, 72.0, 952.0)

    /**
     * The reference declares the petal dimensions in single precision; sampling and software
     * rendering use those values widened to double, exactly as it does.
     */
    internal val PETAL_RADIUS_EXACT = 4.6f.toDouble()
    internal val PETAL_HALF_LENGTH_EXACT = 6.0f.toDouble()
    internal val PETAL_HALF_WIDTH_EXACT = 3.73f.toDouble()
    internal val PETAL_NOTCH_RADIUS_EXACT = 1.12f.toDouble()
    internal val PETAL_NOTCH_DISTANCE_EXACT = 6.24f.toDouble()

    /** Index of the first cell of every band (and the total at the end). */
    internal val BAND_OFFSETS = IntArray(BAND_COUNT + 1)

    /** Band and slot of every field cell. */
    internal val CELL_BAND = IntArray(FIELD_CELLS)
    internal val CELL_SLOT = IntArray(FIELD_CELLS)

    /** Angle of every cell centre (clockwise from 3 o'clock), its cosine and sine, and the centre. */
    internal val CELL_ANGLE = DoubleArray(FIELD_CELLS)
    internal val CELL_COS = DoubleArray(FIELD_CELLS)
    internal val CELL_SIN = DoubleArray(FIELD_CELLS)
    internal val CELL_X = DoubleArray(FIELD_CELLS)
    internal val CELL_Y = DoubleArray(FIELD_CELLS)

    /** Whether every cell lies inside the `天` (drawn in the light colour). */
    internal val CELL_LIGHT = BooleanArray(FIELD_CELLS)

    /** The cells of every sector in bit order: sector `s` occupies `[s·SECTOR_CELLS, (s+1)·SECTOR_CELLS)`. */
    internal val SECTOR_ORDER = IntArray(FIELD_CELLS)

    /** The `#` cells of the mask lattice, row-major `row * MASK_GRID + column`. */
    internal val MASK_CELLS = BooleanArray(MASK_GRID * MASK_GRID)

    /** Role of every flat slot, and its lane `D` bit (`-1` unless DATA). */
    internal val SLOT_ROLES = arrayOfNulls<PetalSlotRole>(TOTAL_SLOTS)
    internal val SLOT_BITS = IntArray(TOTAL_SLOTS)

    /** Ring of every flat slot and its index within the ring. */
    internal val SLOT_RING = IntArray(TOTAL_SLOTS)
    internal val SLOT_INDEX = IntArray(TOTAL_SLOTS)

    /** Decoder slot centres: single-precision like the reference `slot_center`, then widened. */
    internal val SLOT_CENTER_X = DoubleArray(TOTAL_SLOTS)
    internal val SLOT_CENTER_Y = DoubleArray(TOTAL_SLOTS)

    /** Flat slot of every lane `D` bit, in bit order. */
    internal val DATA_SLOTS = IntArray(D_BITS)
    internal val GATE_SLOTS: IntArray
    internal val GUARD_SLOTS: IntArray

    init {
        // Plain loops and fills instead of `IntArray(n) { … }`: Kotlin 2.3.10 generated an
        // unverifiable static initializer (an uninitialised loop counter) for that constructor
        // lambda in this object.
        for (row in 0 until MASK_GRID) {
            val line = MASK[row]
            check(line.length == MASK_GRID) { "mask rows must have $MASK_GRID cells" }
            for (column in 0 until MASK_GRID) MASK_CELLS[row * MASK_GRID + column] = line[column] == '#'
        }
        for (band in 0 until BAND_COUNT) {
            check(BAND_SLOTS[band] % SECTORS == 0) { "band slot counts are multiples of four" }
            BAND_OFFSETS[band + 1] = BAND_OFFSETS[band] + BAND_SLOTS[band]
        }
        check(BAND_OFFSETS[BAND_COUNT] == FIELD_CELLS) { "the band table must hold every cell" }
        for (band in 0 until BAND_COUNT) {
            val slots = BAND_SLOTS[band]
            val radius = bandRadius(band)
            val phase = bandPhase(band)
            for (slot in 0 until slots) {
                val cell = BAND_OFFSETS[band] + slot
                CELL_BAND[cell] = band
                CELL_SLOT[cell] = slot
                val angle = TAU * (slot.toDouble() + phase) / slots.toDouble()
                val cos = StrictMath.cos(angle)
                val sin = StrictMath.sin(angle)
                CELL_ANGLE[cell] = angle
                CELL_COS[cell] = cos
                CELL_SIN[cell] = sin
                CELL_X[cell] = CENTER + radius * cos
                CELL_Y[cell] = CENTER + radius * sin
                CELL_LIGHT[cell] = inMask(CELL_X[cell], CELL_Y[cell])
            }
        }
        var at = 0
        for (sector in 0 until SECTORS) {
            for (band in 0 until BAND_COUNT) {
                val quarter = BAND_SLOTS[band] / SECTORS
                for (slot in sector * quarter until (sector + 1) * quarter) {
                    SECTOR_ORDER[at] = BAND_OFFSETS[band] + slot
                    at += 1
                }
            }
            check(at == (sector + 1) * SECTOR_CELLS) { "sectors hold $SECTOR_CELLS cells" }
        }

        java.util.Arrays.fill(SLOT_BITS, -1)
        for (ring in 0 until RING_COUNT) {
            val offset = ringOffset(ring)
            val n = RING_SLOTS[ring]
            val bases = intArrayOf(0, n / 4, n / 2) // right, bottom, left
            val gates = ArrayList<Int>()
            val guards = ArrayList<Int>()
            for (gate in 0 until 3) {
                val base = bases[gate]
                val first: Int
                val last: Int
                when (GATE_DOTS[gate][ring]) {
                    1 -> { first = base; last = base }
                    2 -> { first = base; last = base + 1 }
                    else -> { first = base + n - 1; last = base + 1 }
                }
                val span = (last + n - first) % n + 1
                for (step in 0 until span) gates += (first + step) % n
                guards += (first + n - 1) % n
                guards += (last + 1) % n
            }
            for (slot in guards) SLOT_ROLES[offset + slot] = PetalSlotRole.GUARD
            for (slot in gates) SLOT_ROLES[offset + slot] = PetalSlotRole.GATE
            for (slot in 0 until n) {
                SLOT_RING[offset + slot] = ring
                SLOT_INDEX[offset + slot] = slot
                // Slot 0 is at 3 o'clock and slots advance clockwise on screen.
                val theta = TAU_F * slot.toFloat() / n.toFloat()
                val cos = StrictMath.cos(theta.toDouble()).toFloat()
                val sin = StrictMath.sin(theta.toDouble()).toFloat()
                SLOT_CENTER_X[offset + slot] = (CENTER_F + RING_RADII_F[ring] * cos).toDouble()
                SLOT_CENTER_Y[offset + slot] = (CENTER_F + RING_RADII_F[ring] * sin).toDouble()
            }
        }
        var next = 0
        for (flat in 0 until TOTAL_SLOTS) {
            if (SLOT_ROLES[flat] == null) {
                if (next < D_BITS) {
                    SLOT_ROLES[flat] = PetalSlotRole.DATA
                    SLOT_BITS[flat] = next
                    DATA_SLOTS[next] = flat
                    next += 1
                } else {
                    SLOT_ROLES[flat] = PetalSlotRole.SPARE
                }
            }
        }
        check(next == D_BITS) { "rings must provide exactly $D_BITS data slots" }
        GATE_SLOTS = (0 until TOTAL_SLOTS).filter { SLOT_ROLES[it] == PetalSlotRole.GATE }.toIntArray()
        GUARD_SLOTS = (0 until TOTAL_SLOTS).filter { SLOT_ROLES[it] == PetalSlotRole.GUARD }.toIntArray()
    }

    private const val TAU_F = 6.2831855f
    private const val CENTER_F = 512f

    /** Number of cells on band [band] (`0..33`). */
    @JvmStatic
    fun bandSlots(band: Int): Int = BAND_SLOTS[checkBand(band)]

    /** Index of the first cell of band [band] (`0..34`; `34` gives [FIELD_CELLS]). */
    @JvmStatic
    fun bandOffset(band: Int): Int {
        require(band in 0..BAND_COUNT) { "band index out of range" }
        return BAND_OFFSETS[band]
    }

    /** Radius of band [band]: `16 + 11 · band`. */
    @JvmStatic
    fun bandRadius(band: Int): Double = BAND_INNER + BAND_SPACING * checkBand(band).toDouble()

    /**
     * Fraction of a slot by which band [band] is turned: odd bands sit half a slot over, so
     * neighbouring bands interleave.
     */
    @JvmStatic
    fun bandPhase(band: Int): Double = if (checkBand(band) % 2 == 1) 0.5 else 0.0

    /** Band of field cell [cell]. */
    @JvmStatic
    fun cellBand(cell: Int): Int = CELL_BAND[checkCell(cell)]

    /** Slot of field cell [cell] within its band. */
    @JvmStatic
    fun cellSlot(cell: Int): Int = CELL_SLOT[checkCell(cell)]

    /** Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell [cell]. */
    @JvmStatic
    fun cellAngle(cell: Int): Double = CELL_ANGLE[checkCell(cell)]

    /** Canvas coordinates `[x, y]` of the centre of field cell [cell]. */
    @JvmStatic
    fun cellCenter(cell: Int): DoubleArray = doubleArrayOf(CELL_X[checkCell(cell)], CELL_Y[cell])

    /** Whether field cell [cell] lies inside the `天` (drawn in the light colour). */
    @JvmStatic
    fun isLightCell(cell: Int): Boolean = CELL_LIGHT[checkCell(cell)]

    /**
     * The [SECTOR_CELLS] cells of sector [sector] (`0..3`) in bit order: band by band from the
     * centre out, and within a band in ascending slot order. Sector `s` holds slots
     * `s·n/4 … (s+1)·n/4 − 1` of every band of `n` slots. The first `8 ×`
     * [PetalLanes.FIELD_WORD] cells carry the sector's codeword; the rest are spare and stay dark.
     */
    @JvmStatic
    fun sectorCells(sector: Int): IntArray {
        require(sector in 0 until SECTORS) { "sector index out of range" }
        return SECTOR_ORDER.copyOfRange(sector * SECTOR_CELLS, (sector + 1) * SECTOR_CELLS)
    }

    /** Whether the canvas point `(x, y)` lies inside the `天` mask. */
    @JvmStatic
    fun inMask(x: Double, y: Double): Boolean {
        if (x < MASK_ORIGIN || y < MASK_ORIGIN) return false
        val column = ((x - MASK_ORIGIN) / MASK_PITCH).toInt()
        val row = ((y - MASK_ORIGIN) / MASK_PITCH).toInt()
        return column < MASK_GRID && row < MASK_GRID && MASK_CELLS[row * MASK_GRID + column]
    }

    /** Radius of ring [ring] (`0..2`) in design units. */
    @JvmStatic
    fun ringRadius(ring: Int): Double = RING_RADII[ring]

    /** Number of dot slots on ring [ring] (`0..2`). */
    @JvmStatic
    fun ringSlots(ring: Int): Int = RING_SLOTS[ring]

    /** Dots in gate [gate] (`0` right, `1` bottom, `2` left) of ring [ring]. */
    @JvmStatic
    fun gateDots(gate: Int, ring: Int): Int = GATE_DOTS[gate][ring]

    /** Offset of ring [ring] inside the flat slot index space. */
    @JvmStatic
    fun ringOffset(ring: Int): Int = when (ring) {
        0 -> 0
        1 -> RING_SLOTS[0]
        else -> RING_SLOTS[0] + RING_SLOTS[1]
    }

    /** Canvas `x` of the centre of corner finder [index] (clockwise from top-left). */
    @JvmStatic
    fun finderCenterX(index: Int): Double = FINDER_CENTERS[2 * checkFinder(index)]

    /** Canvas `y` of the centre of corner finder [index] (clockwise from top-left). */
    @JvmStatic
    fun finderCenterY(index: Int): Double = FINDER_CENTERS[2 * checkFinder(index) + 1]

    /** The role of every ring slot in flat index order. */
    @JvmStatic
    fun slotRoles(): List<PetalSlotRole> = java.util.Collections.unmodifiableList(SLOT_ROLES.map { it!! })

    /** Lane `D` bit carried by flat slot [flat], or `-1` when the slot carries none. */
    @JvmStatic
    fun slotDataBit(flat: Int): Int = SLOT_BITS[checkSlot(flat)]

    /** Flat slot index of every lane `D` bit, in bit order. */
    @JvmStatic
    fun dataSlots(): IntArray = DATA_SLOTS.copyOf()

    /** Flat indices of the always-lit gate slots, ascending. */
    @JvmStatic
    fun gateSlots(): IntArray = GATE_SLOTS.copyOf()

    /** Flat indices of the always-dark guard slots, ascending. */
    @JvmStatic
    fun guardSlots(): IntArray = GUARD_SLOTS.copyOf()

    /** Splits a flat slot index into `[ring, slot]`. */
    @JvmStatic
    fun splitSlot(flat: Int): IntArray = intArrayOf(SLOT_RING[checkSlot(flat)], SLOT_INDEX[flat])

    /**
     * Canvas coordinates `[x, y]` of slot [slot] on ring [ring], as the decoder
     * samples them. Slot `0` is at 3 o'clock and slots advance clockwise.
     */
    @JvmStatic
    fun slotCenter(ring: Int, slot: Int): DoubleArray {
        require(ring in 0 until RING_COUNT && slot in 0 until RING_SLOTS[ring]) { "ring slot out of range" }
        val flat = ringOffset(ring) + slot
        return doubleArrayOf(SLOT_CENTER_X[flat], SLOT_CENTER_Y[flat])
    }

    /**
     * Returns whether the point `(dx, dy)`, relative to a finder centre, is lit.
     *
     * A finder is a solid five-petal sakura blossom whose first petal points
     * straight up. The petal notches are cosmetic; decoders only rely on the
     * blossom being one large, isolated, roughly round blob.
     */
    @JvmStatic
    fun finderLit(dx: Double, dy: Double): Boolean {
        if (Math.sqrt(dx * dx + dy * dy) <= FINDER_CORE) return true
        for (petal in 0 until FINDER_PETALS) {
            val cx = FINDER_PETAL_DISTANCE * PETAL_COS[petal]
            val cy = FINDER_PETAL_DISTANCE * PETAL_SIN[petal]
            val px = dx - cx
            val py = dy - cy
            if (Math.sqrt(px * px + py * py) <= FINDER_PETAL_RADIUS) {
                val nx = dx - FINDER_OUTER * PETAL_COS[petal]
                val ny = dy - FINDER_OUTER * PETAL_SIN[petal]
                return Math.sqrt(nx * nx + ny * ny) > FINDER_NOTCH_RADIUS
            }
        }
        return false
    }

    /**
     * Returns whether the point `(dx, dy)`, relative to the centre of a petal that points along
     * [angle], is lit: an ellipse of half-length [PETAL_HALF_LENGTH] along [angle] and half-width
     * [PETAL_HALF_WIDTH], minus the notch at its tip.
     */
    @JvmStatic
    fun petalLit(dx: Double, dy: Double, angle: Double): Boolean =
        petalLit(dx, dy, StrictMath.sin(angle), StrictMath.cos(angle))

    /** [petalLit] for a petal direction given by its sine and cosine. */
    internal fun petalLit(dx: Double, dy: Double, sin: Double, cos: Double): Boolean {
        val along = dx * cos + dy * sin
        val across = -dx * sin + dy * cos
        val a = along / PETAL_HALF_LENGTH_EXACT
        val b = across / PETAL_HALF_WIDTH_EXACT
        if (a * a + b * b > 1.0) return false
        val notch = along - PETAL_NOTCH_DISTANCE_EXACT
        return notch * notch + across * across > PETAL_NOTCH_RADIUS_EXACT * PETAL_NOTCH_RADIUS_EXACT
    }

    /** Cosine and sine of each finder petal direction (`-π/2 + τ·p/5`). */
    internal val PETAL_COS = DoubleArray(FINDER_PETALS)
    internal val PETAL_SIN = DoubleArray(FINDER_PETALS)

    init {
        for (petal in 0 until FINDER_PETALS) {
            val angle = -FRAC_PI_2 + TAU * petal.toDouble() / FINDER_PETALS.toDouble()
            PETAL_COS[petal] = StrictMath.cos(angle)
            PETAL_SIN[petal] = StrictMath.sin(angle)
        }
    }

    /** `τ = 2π` in double precision. */
    internal const val TAU = 6.283185307179586
    private const val FRAC_PI_2 = 1.5707963267948966

    private fun checkBand(band: Int): Int {
        require(band in 0 until BAND_COUNT) { "band index out of range" }
        return band
    }

    private fun checkCell(cell: Int): Int {
        require(cell in 0 until FIELD_CELLS) { "field cell index out of range" }
        return cell
    }

    private fun checkSlot(flat: Int): Int {
        require(flat in 0 until TOTAL_SLOTS) { "slot index out of range" }
        return flat
    }

    private fun checkFinder(index: Int): Int {
        require(index in 0 until 4) { "finder index out of range" }
        return index
    }
}
