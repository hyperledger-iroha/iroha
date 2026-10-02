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
 * Normative Petal frame geometry.
 *
 * All coordinates are *design units* on a square canvas of [CANVAS] units with
 * the origin at the top-left and `y` growing downward. A renderer scales the
 * canvas to any pixel size; a decoder maps camera pixels back to design units.
 * Cell *order* is defined by integer indices only.
 */
object PetalLayout {
    /** Canvas side length in design units. */
    const val CANVAS = 1024.0

    /** Canvas centre coordinate on both axes. */
    const val CENTER = 512.0

    /** Number of tile lattice columns and rows. */
    const val TILE_GRID = 20

    /** Canvas coordinate of the lattice's left and top edge. */
    const val TILE_ORIGIN = 222.0

    /** Lattice pitch in design units. */
    const val TILE_PITCH = 29.0

    /** Side of a drawn tile in design units (pitch minus a 4-unit gutter). */
    const val TILE_SIZE = 25.0

    /** Corner radius of a drawn tile in design units. */
    const val TILE_CORNER_RADIUS = 3.0

    /** Side of the square glyph box inside a tile. */
    const val GLYPH_BOX = 23.0

    /** Number of data tiles in the `天` mask. */
    const val TILE_COUNT = 256

    /** Number of concentric dot rings. */
    const val RING_COUNT = 3

    /** Radius of a drawn ring dot. */
    const val DOT_RADIUS = 11.0

    /** Total dot slots over the three rings. */
    const val TOTAL_SLOTS = 80 + 92 + 104

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
     * marks a data tile. The mask is mirror-symmetric left to right and not
     * symmetric top to bottom, so it also tells a decoder which way is up.
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

    private val RING_RADII = doubleArrayOf(360.0, 410.0, 460.0)
    private val RING_RADII_F = floatArrayOf(360f, 410f, 460f)
    private val RING_SLOTS = intArrayOf(80, 92, 104)

    /** Dots per gate and ring for the right, bottom and left gates (no top gate). */
    private val GATE_DOTS = arrayOf(intArrayOf(1, 1, 3), intArrayOf(2, 2, 2), intArrayOf(1, 2, 2))
    private val FINDER_CENTERS = doubleArrayOf(72.0, 72.0, 952.0, 72.0, 952.0, 952.0, 72.0, 952.0)

    /** Lattice column and row of every tile, row-major. */
    internal val TILE_COLUMNS = IntArray(TILE_COUNT)
    internal val TILE_ROWS = IntArray(TILE_COUNT)

    /** Tile centres in canvas units. */
    internal val TILE_CENTER_X = DoubleArray(TILE_COUNT)
    internal val TILE_CENTER_Y = DoubleArray(TILE_COUNT)

    /** Tile index at `row * TILE_GRID + column`, or `-1`. */
    internal val TILE_AT = IntArray(TILE_GRID * TILE_GRID)

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
        // Plain fills instead of `IntArray(n) { -1 }`: Kotlin 2.3.10 generated an
        // unverifiable static initializer (an uninitialised loop counter) for
        // that constructor lambda in this object.
        java.util.Arrays.fill(TILE_AT, -1)
        java.util.Arrays.fill(SLOT_BITS, -1)
        var count = 0
        for (row in 0 until TILE_GRID) {
            val line = MASK[row]
            check(line.length == TILE_GRID) { "mask rows must have $TILE_GRID cells" }
            for (column in 0 until TILE_GRID) {
                if (line[column] == '#') {
                    check(count < TILE_COUNT) { "mask must contain exactly $TILE_COUNT tiles" }
                    TILE_COLUMNS[count] = column
                    TILE_ROWS[count] = row
                    // The reference computes these in f32; they are exact in both precisions.
                    TILE_CENTER_X[count] = TILE_ORIGIN + TILE_PITCH * (column + 0.5)
                    TILE_CENTER_Y[count] = TILE_ORIGIN + TILE_PITCH * (row + 0.5)
                    TILE_AT[row * TILE_GRID + column] = count
                    count += 1
                }
            }
        }
        check(count == TILE_COUNT) { "mask must contain exactly $TILE_COUNT tiles" }

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

    /** Lattice column of tile [index]. */
    @JvmStatic
    fun tileColumn(index: Int): Int = TILE_COLUMNS[checkTile(index)]

    /** Lattice row of tile [index]. */
    @JvmStatic
    fun tileRow(index: Int): Int = TILE_ROWS[checkTile(index)]

    /** Canvas coordinates `[x, y]` of the centre of tile [index]. */
    @JvmStatic
    fun tileCenter(index: Int): DoubleArray = doubleArrayOf(TILE_CENTER_X[checkTile(index)], TILE_CENTER_Y[index])

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

    /** Cosine and sine of each petal direction (`-π/2 + τ·p/5`). */
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

    private fun checkTile(index: Int): Int {
        require(index in 0 until TILE_COUNT) { "tile index out of range" }
        return index
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
