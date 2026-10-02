package org.hyperledger.iroha.sdk.offline.petal

/** Colours of a Petal picture, each a packed `0xRRGGBB` value. Immutable. */
class PetalPalette(
    /** Background. */
    val background: Int,
    /** Light tile fill and finders. */
    val light: Int,
    /** Sakura pink of dots and of glyphs on dark tiles. */
    val pink: Int,
    /** Glyph colour on a light tile. */
    val ink: Int,
) {
    init {
        require(listOf(background, light, pink, ink).all { it in 0..0xFF_FFFF }) { "palette colours are 0xRRGGBB" }
    }

    override fun equals(other: Any?): Boolean = other is PetalPalette && background == other.background &&
        light == other.light && pink == other.pink && ink == other.ink

    override fun hashCode(): Int = 31 * (31 * (31 * background + light) + pink) + ink

    companion object {
        /** Black background, `(250, 235, 244)` light, `(245, 175, 208)` pink, `(20, 4, 14)` ink. */
        @JvmField
        val DEFAULT = PetalPalette(0x000000, 0xFAEBF4, 0xF5AFD0, 0x14040E)
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
 * The picture is black, with four sakura-blossom finders in the canvas
 * corners and a `天`-shaped field of 256 tiles inside three dotted rings. A
 * light tile is a pale rounded square with a near-black katakana; a dark tile
 * is empty except for its sakura-pink katakana. Platform code may draw with its
 * own 2D API from a [PetalDrawList] as long as geometry and polarity match.
 */
object PetalRenderer {
    private const val BITMAP_N = 128

    /** Glyph ink bitmaps, `128 × 128` per glyph, built on first use. */
    private val bitmaps: BooleanArray by lazy {
        val out = BooleanArray(PetalGlyphs.GLYPH_COUNT * BITMAP_N * BITMAP_N)
        for (glyph in 0 until PetalGlyphs.GLYPH_COUNT) {
            val base = glyph * BITMAP_N * BITMAP_N
            for (v in 0 until BITMAP_N) {
                for (u in 0 until BITMAP_N) {
                    val x = (u.toDouble() + 0.5) / BITMAP_N.toDouble() * PetalGlyphs.GLYPH_GRID
                    val y = (v.toDouble() + 0.5) / BITMAP_N.toDouble() * PetalGlyphs.GLYPH_GRID
                    out[base + v * BITMAP_N + u] = PetalGlyphs.isInked(glyph, x, y)
                }
            }
        }
        out
    }

    /** Renders one frame into an RGB image. */
    @JvmStatic
    @JvmOverloads
    fun render(cells: PetalFrameCells, options: PetalRenderOptions = PetalRenderOptions.DEFAULT): PetalRgb {
        val size = options.size
        val s = options.supersample
        val palette = options.palette
        val glyphs = bitmaps
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
                        val colour = shade(cells, palette, glyphs, x, y)
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

    /** Inside test for a rounded square of half-side [half] and corner [radius], relative to its centre. */
    private fun inRoundedSquare(dx: Double, dy: Double, half: Double, radius: Double): Boolean {
        val ax = Math.abs(dx)
        val ay = Math.abs(dy)
        if (ax > half || ay > half) return false
        val cx = ax - (half - radius)
        val cy = ay - (half - radius)
        return cx <= 0.0 || cy <= 0.0 || cx * cx + cy * cy <= radius * radius
    }

    private fun shade(cells: PetalFrameCells, palette: PetalPalette, glyphs: BooleanArray, x: Double, y: Double): Int {
        // finders
        for (finder in 0 until 4) {
            val dx = x - PetalLayout.finderCenterX(finder)
            val dy = y - PetalLayout.finderCenterY(finder)
            if (Math.sqrt(dx * dx + dy * dy) <= PetalLayout.FINDER_OUTER) {
                return if (PetalLayout.finderLit(dx, dy)) palette.light else palette.background
            }
        }
        // tiles
        val lattice = PetalLayout.TILE_ORIGIN
        val pitch = PetalLayout.TILE_PITCH
        if (x >= lattice && y >= lattice) {
            val column = ((x - lattice) / pitch).toInt()
            val row = ((y - lattice) / pitch).toInt()
            if (column < PetalLayout.TILE_GRID && row < PetalLayout.TILE_GRID) {
                val tile = PetalLayout.TILE_AT[row * PetalLayout.TILE_GRID + column]
                if (tile >= 0) {
                    val cx = lattice + pitch * (column.toDouble() + 0.5)
                    val cy = lattice + pitch * (row.toDouble() + 0.5)
                    val dx = x - cx
                    val dy = y - cy
                    if (!inRoundedSquare(dx, dy, PetalLayout.TILE_SIZE / 2.0, PetalLayout.TILE_CORNER_RADIUS)) {
                        return palette.background
                    }
                    val boxHalf = PetalLayout.GLYPH_BOX / 2.0
                    val inked = Math.abs(dx) < boxHalf && Math.abs(dy) < boxHalf && run {
                        val u = ((dx + boxHalf) / (2.0 * boxHalf) * BITMAP_N.toDouble()).toInt()
                        val v = ((dy + boxHalf) / (2.0 * boxHalf) * BITMAP_N.toDouble()).toInt()
                        glyphs[cells.glyph(tile) * BITMAP_N * BITMAP_N + minOf(v, BITMAP_N - 1) * BITMAP_N + minOf(u, BITMAP_N - 1)]
                    }
                    val light = cells.isLight(tile)
                    return when {
                        light && !inked -> palette.light
                        light -> palette.ink
                        inked -> palette.pink
                        else -> palette.background
                    }
                }
            }
        }
        // ring dots
        val dx = x - PetalLayout.CENTER
        val dy = y - PetalLayout.CENTER
        val radius = Math.sqrt(dx * dx + dy * dy)
        for (ring in 0 until PetalLayout.RING_COUNT) {
            val ringRadius = PetalLayout.ringRadius(ring)
            if (Math.abs(radius - ringRadius) > PetalLayout.DOT_RADIUS) continue
            val slots = PetalLayout.ringSlots(ring)
            var theta = StrictMath.atan2(dy, dx)
            if (theta < 0.0) theta += PetalLayout.TAU
            val slot = PetalNumerics.roundHalfAway(theta / PetalLayout.TAU * slots.toDouble()).toInt() % slots
            if (!cells.isDotLit(PetalLayout.ringOffset(ring) + slot)) continue
            val angle = PetalLayout.TAU * slot.toDouble() / slots.toDouble()
            val ex = dx - ringRadius * StrictMath.cos(angle)
            val ey = dy - ringRadius * StrictMath.sin(angle)
            if (Math.sqrt(ex * ex + ey * ey) <= PetalLayout.DOT_RADIUS) return palette.pink
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

/** One tile of a [PetalDrawList], in canvas design units. */
class PetalDrawTile(
    /** Tile index (`0..255`). */
    val index: Int,
    /** Centre `x`. */
    val centerX: Double,
    /** Centre `y`. */
    val centerY: Double,
    /** Whether the tile is filled with the light colour. */
    val light: Boolean,
    /** Glyph symbol (`0..15`); see [PetalGlyphs.strokes]. */
    val glyph: Int,
) {
    /** Left edge of the glyph box ([PetalLayout.GLYPH_BOX] units square). */
    val glyphLeft: Double get() = centerX - PetalLayout.GLYPH_BOX / 2.0

    /** Top edge of the glyph box. */
    val glyphTop: Double get() = centerY - PetalLayout.GLYPH_BOX / 2.0
}

/**
 * A platform-neutral vector description of one frame for native 2D canvases.
 *
 * Coordinates are canvas design units (`0..1024`, `y` down); scale them to the
 * output size. Paint in this order:
 *
 * 1. fill the canvas with [PetalPalette.background];
 * 2. fill [finderDiscs] (centre disc and five petals per finder) with
 *    [PetalPalette.light], then [finderNotches] with the background colour;
 * 3. for every tile, fill a [PetalLayout.TILE_SIZE] square with corner radius
 *    [PetalLayout.TILE_CORNER_RADIUS] centred on the tile with the light
 *    colour when [PetalDrawTile.light]; then stroke the glyph polylines of
 *    [PetalGlyphs.strokes] (32-unit grid) scaled by [GLYPH_SCALE] from
 *    `(glyphLeft, glyphTop)`, clipped to the glyph box, with round caps and
 *    joins and width [GLYPH_STROKE_WIDTH], in [PetalPalette.ink] on light tiles
 *    and [PetalPalette.pink] on dark tiles;
 * 4. fill [dots] with [PetalPalette.pink].
 */
class PetalDrawList private constructor(
    /** Colours. */
    val palette: PetalPalette,
    /** Finder core discs and petals, filled with the light colour. */
    val finderDiscs: List<PetalCircle>,
    /** Petal-tip notches, filled with the background colour after [finderDiscs]. */
    val finderNotches: List<PetalCircle>,
    /** All 256 tiles in tile order. */
    val tiles: List<PetalDrawTile>,
    /** Lit ring dots (gates included), filled with the pink colour. */
    val dots: List<PetalCircle>,
) {
    /** Glyph colour of [tile]: ink on light tiles, pink on dark ones. */
    fun glyphColor(tile: PetalDrawTile): Int = if (tile.light) palette.ink else palette.pink

    companion object {
        /** Canvas side in design units. */
        const val CANVAS = PetalLayout.CANVAS

        /** Scale from the 32-unit glyph grid to canvas units (`23 / 32`). */
        const val GLYPH_SCALE = PetalLayout.GLYPH_BOX / PetalGlyphs.GLYPH_GRID

        /** Glyph stroke width in canvas units (`6.5 / 32` of the glyph box). */
        const val GLYPH_STROKE_WIDTH = PetalGlyphs.STROKE_WIDTH * GLYPH_SCALE

        private val FINDER_DISCS: List<PetalCircle>
        private val FINDER_NOTCHES: List<PetalCircle>

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
        }

        /** Builds the draw list of [cells]. */
        @JvmStatic
        @JvmOverloads
        fun of(cells: PetalFrameCells, palette: PetalPalette = PetalPalette.DEFAULT): PetalDrawList {
            val tiles = ArrayList<PetalDrawTile>(PetalLayout.TILE_COUNT)
            for (tile in 0 until PetalLayout.TILE_COUNT) {
                tiles += PetalDrawTile(
                    tile,
                    PetalLayout.TILE_CENTER_X[tile],
                    PetalLayout.TILE_CENTER_Y[tile],
                    cells.isLight(tile),
                    cells.glyph(tile),
                )
            }
            val dots = ArrayList<PetalCircle>()
            for (ring in 0 until PetalLayout.RING_COUNT) {
                val slots = PetalLayout.ringSlots(ring)
                val radius = PetalLayout.ringRadius(ring)
                for (slot in 0 until slots) {
                    if (!cells.isDotLit(PetalLayout.ringOffset(ring) + slot)) continue
                    val angle = PetalLayout.TAU * slot.toDouble() / slots.toDouble()
                    dots += PetalCircle(
                        PetalLayout.CENTER + radius * StrictMath.cos(angle),
                        PetalLayout.CENTER + radius * StrictMath.sin(angle),
                        PetalLayout.DOT_RADIUS,
                    )
                }
            }
            return PetalDrawList(
                palette,
                FINDER_DISCS,
                FINDER_NOTCHES,
                java.util.Collections.unmodifiableList(tiles),
                java.util.Collections.unmodifiableList(dots),
            )
        }
    }
}
