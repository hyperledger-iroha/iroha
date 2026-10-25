package org.hyperledger.iroha.sdk.offline.petal

/** Colours of a Petal picture, each a packed `0xRRGGBB` value. Immutable. */
class PetalPalette(
    /** Background. */
    val background: Int,
    /** Finders and the petals inside the `天`. */
    val light: Int,
    /** Sakura pink of the ring dots and of the petals outside the `天`. */
    val pink: Int,
    /** The faint `天` drawn under the petals (a tenth of the light colour). */
    val backdrop: Int,
) {
    init {
        require(listOf(background, light, pink, backdrop).all { it in 0..0xFF_FFFF }) { "palette colours are 0xRRGGBB" }
    }

    override fun equals(other: Any?): Boolean = other is PetalPalette && background == other.background &&
        light == other.light && pink == other.pink && backdrop == other.backdrop

    override fun hashCode(): Int = 31 * (31 * (31 * background + light) + pink) + backdrop

    companion object {
        /** Black background, `(250, 235, 244)` light, `(245, 175, 208)` pink, `(25, 24, 24)` backdrop. */
        @JvmField
        val DEFAULT = PetalPalette(0x000000, 0xFAEBF4, 0xF5AFD0, 0x191818)
    }
}

/** Software rendering options. */
class PetalRenderOptions @JvmOverloads constructor(
    /** Output side in pixels. */
    val size: Int = 1024,
    /** Samples per pixel side for anti-aliasing (`1..4`). */
    val supersample: Int = 3,
    /** Colours. */
    val palette: PetalPalette = PetalPalette.DEFAULT,
) {
    init {
        require(size > 0 && size.toLong() * size.toLong() * 3 <= Int.MAX_VALUE) { "render size out of range" }
        require(supersample in 1..4) { "supersample must be 1..4" }
    }

    companion object {
        /** 1024 pixels, 3×3 supersampling, default palette. */
        @JvmField
        val DEFAULT = PetalRenderOptions()
    }
}

/**
 * Reference software renderer: equivalent to the Rust `render` module.
 *
 * The picture is black, with four sakura-blossom finders in the canvas corners,
 * three dotted rings, and inside them a field of petals on concentric bands. A
 * lit petal whose centre lies inside the `天` is drawn in the light colour, the
 * others in sakura pink, over a faint backdrop of the `天` itself, so the
 * character shows through the storm of petals. Platform code may draw with its
 * own 2D API from a [PetalDrawList] as long as geometry and polarity match.
 */
object PetalRenderer {
    /** Renders one frame into an RGB image. */
    @JvmStatic
    @JvmOverloads
    fun render(cells: PetalFrameCells, options: PetalRenderOptions = PetalRenderOptions.DEFAULT): PetalRgb {
        val size = options.size
        val s = options.supersample
        val palette = options.palette
        val petals = cells.petalView
        val unit = PetalLayout.CANVAS / size.toDouble()
        val data = ByteArray(size * size * 3)
        val n = s * s
        for (py in 0 until size) {
            for (px in 0 until size) {
                var r = 0
                var g = 0
                var b = 0
                for (sy in 0 until s) {
                    for (sx in 0 until s) {
                        val x = (px.toDouble() + (sx.toDouble() + 0.5) / s.toDouble()) * unit
                        val y = (py.toDouble() + (sy.toDouble() + 0.5) / s.toDouble()) * unit
                        val colour = shade(cells, petals, palette, x, y)
                        r += colour ushr 16
                        g += (colour ushr 8) and 0xFF
                        b += colour and 0xFF
                    }
                }
                val at = (py * size + px) * 3
                data[at] = ((r + n / 2) / n).toByte()
                data[at + 1] = ((g + n / 2) / n).toByte()
                data[at + 2] = ((b + n / 2) / n).toByte()
            }
        }
        return PetalRgb.wrap(size, size, data)
    }

    /** Builds the vector draw list of [cells] for a native 2D canvas. */
    @JvmStatic
    @JvmOverloads
    fun drawList(cells: PetalFrameCells, palette: PetalPalette = PetalPalette.DEFAULT): PetalDrawList =
        PetalDrawList.of(cells, palette)

    /** The colour of the lit petal that covers `(x, y)`, or `-1`. */
    private fun petal(petals: BooleanArray, palette: PetalPalette, x: Double, y: Double, radius: Double, theta: Double): Int {
        val reach = PetalLayout.PETAL_HALF_LENGTH_EXACT
        val nearest = PetalNumerics.roundHalfAway((radius - PetalLayout.BAND_INNER) / PetalLayout.BAND_SPACING)
        for (offset in -1..1) {
            val candidate = nearest + offset.toDouble()
            if (candidate < 0.0 || candidate >= PetalLayout.BAND_COUNT.toDouble()) continue
            val band = candidate.toInt()
            if (Math.abs(radius - PetalLayout.bandRadius(band)) > reach) continue
            val count = PetalLayout.bandSlots(band)
            val slots = count.toDouble()
            val around = PetalNumerics.roundHalfAway(theta / PetalLayout.TAU * slots - PetalLayout.bandPhase(band)).toInt()
            for (step in -1..1) {
                val cell = PetalLayout.BAND_OFFSETS[band] + Math.floorMod(around + step, count)
                if (!petals[cell]) continue
                val lit = PetalLayout.petalLit(
                    x - PetalLayout.CELL_X[cell],
                    y - PetalLayout.CELL_Y[cell],
                    PetalLayout.CELL_SIN[cell],
                    PetalLayout.CELL_COS[cell],
                )
                if (lit) return if (PetalLayout.CELL_LIGHT[cell]) palette.light else palette.pink
            }
        }
        return -1
    }

    private fun shade(cells: PetalFrameCells, petals: BooleanArray, palette: PetalPalette, x: Double, y: Double): Int {
        // finders
        for (finder in 0 until 4) {
            val dx = x - PetalLayout.finderCenterX(finder)
            val dy = y - PetalLayout.finderCenterY(finder)
            if (Math.sqrt(dx * dx + dy * dy) <= PetalLayout.FINDER_OUTER) {
                return if (PetalLayout.finderLit(dx, dy)) palette.light else palette.background
            }
        }
        val dx = x - PetalLayout.CENTER
        val dy = y - PetalLayout.CENTER
        val radius = Math.sqrt(dx * dx + dy * dy)
        var theta = StrictMath.atan2(dy, dx)
        if (theta < 0.0) theta += PetalLayout.TAU
        // ring dots
        for (ring in 0 until PetalLayout.RING_COUNT) {
            val ringRadius = PetalLayout.ringRadius(ring)
            if (Math.abs(radius - ringRadius) > PetalLayout.DOT_RADIUS) continue
            val slots = PetalLayout.ringSlots(ring)
            val slot = PetalNumerics.roundHalfAway(theta / PetalLayout.TAU * slots.toDouble()).toInt() % slots
            if (!cells.isDotLit(PetalLayout.ringOffset(ring) + slot)) continue
            val angle = PetalLayout.TAU * slot.toDouble() / slots.toDouble()
            val ex = dx - ringRadius * StrictMath.cos(angle)
            val ey = dy - ringRadius * StrictMath.sin(angle)
            if (Math.sqrt(ex * ex + ey * ey) <= PetalLayout.DOT_RADIUS) return palette.pink
        }
        // petals, then the faint 天 under them
        if (radius <= PetalLayout.BACKDROP_RADIUS) {
            val colour = petal(petals, palette, x, y, radius, theta)
            if (colour >= 0) return colour
            if (PetalLayout.inMask(x, y)) return palette.backdrop
        }
        return palette.background
    }
}

/** A filled circle in canvas design units. */
class PetalCircle(
    /** Centre `x`. */
    val x: Double,
    /** Centre `y`. */
    val y: Double,
    /** Radius. */
    val radius: Double,
)

/** An axis-aligned rectangle in canvas design units. */
class PetalRect(
    /** Left edge. */
    val left: Double,
    /** Top edge. */
    val top: Double,
    /** Right edge. */
    val right: Double,
    /** Bottom edge. */
    val bottom: Double,
)

/** One lit petal of a [PetalDrawList], in canvas design units. */
class PetalDrawPetal(
    /** Field cell index (`0..3375`). */
    val cell: Int,
    /** Centre `x`. */
    val centerX: Double,
    /** Centre `y`. */
    val centerY: Double,
    /** Outward direction of the petal: radians, clockwise on the screen from 3 o'clock. */
    val angle: Double,
    /** Whether the petal lies inside the `天` and is drawn in the light colour (pink otherwise). */
    val light: Boolean,
)

/**
 * A platform-neutral vector description of one frame for native 2D canvases.
 *
 * Coordinates are canvas design units (`0..1024`, `y` down); scale them to the
 * output size. Paint in this order:
 *
 * 1. fill the canvas with [PetalPalette.background];
 * 2. fill [finderDiscs] (centre disc and five petals per finder) with
 *    [PetalPalette.light], then [finderNotches] with the background colour;
 * 3. fill [backdropCells] with [PetalPalette.backdrop], clipped to the disc of
 *    radius [BACKDROP_RADIUS] around the canvas centre (a circular clip is fine);
 * 4. fill every one of [petals] in [petalColor]: an ellipse centred on the petal
 *    with half-length [PETAL_HALF_LENGTH] along [PetalDrawPetal.angle] and
 *    half-width [PETAL_HALF_WIDTH], minus the cosmetic notch (a disc of radius
 *    [PETAL_NOTCH_RADIUS] centred [PETAL_NOTCH_DISTANCE] outward from the petal
 *    centre), which may be omitted;
 * 5. fill [dots] with [PetalPalette.pink].
 *
 * The finder and backdrop lists and the petal and dot shapes are shared between
 * frames, so a list costs two small arrays per frame.
 */
class PetalDrawList private constructor(
    /** Colours. */
    val palette: PetalPalette,
    /** Finder core discs and petals, filled with the light colour. */
    val finderDiscs: List<PetalCircle>,
    /** Petal-tip notches, filled with the background colour after [finderDiscs]. */
    val finderNotches: List<PetalCircle>,
    /** The `#` cells of the `天` lattice, filled with the backdrop colour inside [BACKDROP_RADIUS]. */
    val backdropCells: List<PetalRect>,
    /** Lit petals in field cell order. */
    val petals: List<PetalDrawPetal>,
    /** Lit ring dots (gates included), filled with the pink colour. */
    val dots: List<PetalCircle>,
) {
    /** Colour of [petal]: light inside the `天`, pink outside. */
    fun petalColor(petal: PetalDrawPetal): Int = if (petal.light) palette.light else palette.pink

    companion object {
        /** Canvas side in design units. */
        const val CANVAS = PetalLayout.CANVAS

        /** Radius of the disc around the canvas centre that clips the backdrop. */
        const val BACKDROP_RADIUS = PetalLayout.BACKDROP_RADIUS

        /** Half the length of a petal, along its outward direction. */
        const val PETAL_HALF_LENGTH = PetalLayout.PETAL_HALF_LENGTH

        /** Half the width of a petal. */
        const val PETAL_HALF_WIDTH = PetalLayout.PETAL_HALF_WIDTH

        /** Radius of the cosmetic notch at a petal tip. */
        const val PETAL_NOTCH_RADIUS = PetalLayout.PETAL_NOTCH_RADIUS

        /** Distance from a petal centre to its notch centre, outward. */
        const val PETAL_NOTCH_DISTANCE = PetalLayout.PETAL_NOTCH_DISTANCE

        private val FINDER_DISCS: List<PetalCircle>
        private val FINDER_NOTCHES: List<PetalCircle>
        private val BACKDROP_CELLS: List<PetalRect>

        /** The petal of every field cell and the dot of every ring slot, shared by all lists. */
        private val CELL_PETALS: Array<PetalDrawPetal>
        private val SLOT_DOTS: Array<PetalCircle>

        init {
            val discs = ArrayList<PetalCircle>()
            val notches = ArrayList<PetalCircle>()
            for (finder in 0 until 4) {
                val fx = PetalLayout.finderCenterX(finder)
                val fy = PetalLayout.finderCenterY(finder)
                discs += PetalCircle(fx, fy, PetalLayout.FINDER_CORE)
                for (petal in 0 until PetalLayout.FINDER_PETALS) {
                    val cos = PetalLayout.PETAL_COS[petal]
                    val sin = PetalLayout.PETAL_SIN[petal]
                    discs += PetalCircle(
                        fx + PetalLayout.FINDER_PETAL_DISTANCE * cos,
                        fy + PetalLayout.FINDER_PETAL_DISTANCE * sin,
                        PetalLayout.FINDER_PETAL_RADIUS,
                    )
                    notches += PetalCircle(
                        fx + PetalLayout.FINDER_OUTER * cos,
                        fy + PetalLayout.FINDER_OUTER * sin,
                        PetalLayout.FINDER_NOTCH_RADIUS,
                    )
                }
            }
            FINDER_DISCS = java.util.Collections.unmodifiableList(discs)
            FINDER_NOTCHES = java.util.Collections.unmodifiableList(notches)
            val backdrop = ArrayList<PetalRect>()
            for (row in 0 until PetalLayout.MASK_GRID) {
                for (column in 0 until PetalLayout.MASK_GRID) {
                    if (!PetalLayout.MASK_CELLS[row * PetalLayout.MASK_GRID + column]) continue
                    val left = PetalLayout.MASK_ORIGIN + PetalLayout.MASK_PITCH * column.toDouble()
                    val top = PetalLayout.MASK_ORIGIN + PetalLayout.MASK_PITCH * row.toDouble()
                    backdrop += PetalRect(left, top, left + PetalLayout.MASK_PITCH, top + PetalLayout.MASK_PITCH)
                }
            }
            BACKDROP_CELLS = java.util.Collections.unmodifiableList(backdrop)
            CELL_PETALS = Array(PetalLayout.FIELD_CELLS) { cell ->
                PetalDrawPetal(
                    cell,
                    PetalLayout.CELL_X[cell],
                    PetalLayout.CELL_Y[cell],
                    PetalLayout.CELL_ANGLE[cell],
                    PetalLayout.CELL_LIGHT[cell],
                )
            }
            SLOT_DOTS = Array(PetalLayout.TOTAL_SLOTS) { flat ->
                val ring = PetalLayout.SLOT_RING[flat]
                val slots = PetalLayout.ringSlots(ring)
                val radius = PetalLayout.ringRadius(ring)
                val angle = PetalLayout.TAU * PetalLayout.SLOT_INDEX[flat].toDouble() / slots.toDouble()
                PetalCircle(
                    PetalLayout.CENTER + radius * StrictMath.cos(angle),
                    PetalLayout.CENTER + radius * StrictMath.sin(angle),
                    PetalLayout.DOT_RADIUS,
                )
            }
        }

        /** Builds the draw list of [cells]. */
        @JvmStatic
        @JvmOverloads
        fun of(cells: PetalFrameCells, palette: PetalPalette = PetalPalette.DEFAULT): PetalDrawList {
            val lit = cells.petalView
            val petals = ArrayList<PetalDrawPetal>(PetalLayout.FIELD_CELLS / 2)
            for (cell in 0 until PetalLayout.FIELD_CELLS) if (lit[cell]) petals += CELL_PETALS[cell]
            val dots = ArrayList<PetalCircle>()
            for (flat in 0 until PetalLayout.TOTAL_SLOTS) if (cells.isDotLit(flat)) dots += SLOT_DOTS[flat]
            return PetalDrawList(
                palette,
                FINDER_DISCS,
                FINDER_NOTCHES,
                BACKDROP_CELLS,
                java.util.Collections.unmodifiableList(petals),
                java.util.Collections.unmodifiableList(dots),
            )
        }
    }
}
