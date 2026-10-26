package org.hyperledger.iroha.sdk.offline.petal

/** Colours of a Petal picture, each a packed `0xRRGGBB` value. Immutable. */
class PetalPalette @JvmOverloads constructor(
    /** Background. */
    val background: Int,
    /** Finders, the petals inside the `天` and the light katakana tiles. */
    val light: Int,
    /** Sakura pink of the ring dots, of the petals outside the `天` and of the glyphs on dark tiles. */
    val pink: Int,
    /** The faint `天` drawn under the petals (a tenth of the light colour). */
    val backdrop: Int,
    /** Glyphs on light katakana tiles. */
    val ink: Int = 0x14040E,
) {
    init {
        require(listOf(background, light, pink, backdrop, ink).all { it in 0..0xFF_FFFF }) { "palette colours are 0xRRGGBB" }
    }

    override fun equals(other: Any?): Boolean = other is PetalPalette && background == other.background &&
        light == other.light && pink == other.pink && backdrop == other.backdrop && ink == other.ink

    override fun hashCode(): Int = 31 * (31 * (31 * (31 * background + light) + pink) + backdrop) + ink

    companion object {
        /**
         * Black background, `(250, 235, 244)` light, `(245, 175, 208)` pink, `(25, 24, 24)` backdrop
         * and `(20, 4, 14)` ink.
         */
        @JvmField
        val DEFAULT = PetalPalette(0x000000, 0xFAEBF4, 0xF5AFD0, 0x191818, 0x14040E)
    }
}

/** Software rendering options. */
class PetalRenderOptions @JvmOverloads constructor(
    /** Output width in pixels; the card is [PetalRenderer.cardHeightPx] pixels tall. */
    val size: Int = 1024,
    /** Samples per pixel side for anti-aliasing (`1..4`). */
    val supersample: Int = 3,
    /** Colours. */
    val palette: PetalPalette = PetalPalette.DEFAULT,
) {
    init {
        require(size > 0 && size.toLong() * PetalRenderer.cardHeightPx(size).toLong() * 3 <= Int.MAX_VALUE) {
            "render size out of range"
        }
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
 * three dotted rings, and inside them a field of petals on concentric bands, each
 * lit petal turned about its centre by its turn. A lit petal whose centre lies
 * inside the `天` is drawn in the light colour, the others in sakura pink, over a
 * faint backdrop of the `天` itself, so the character shows through the storm of
 * petals. Above and below that square, the card's two bands show rows of katakana
 * tiles (see [PetalKana]): a light tile filled light with its glyph in ink, a dark
 * tile black with its glyph in pink. Platform code may draw with its own 2D API
 * from a [PetalDrawList] as long as geometry, turns, glyphs and polarity match.
 */
object PetalRenderer {
    /** Height in pixels of a card rendered [size] pixels wide: `size · 1668 / 1024`, rounded. */
    @JvmStatic
    fun cardHeightPx(size: Int): Int {
        val height = size.toDouble() * PetalLayout.CARD_HEIGHT / PetalLayout.CANVAS
        return Math.floor(height + 0.5).toInt()
    }

    /**
     * Renders one frame: the card, [PetalRenderOptions.size] pixels wide and [cardHeightPx] tall,
     * whose square starts [PetalLayout.SQUARE_TOP] units below the top edge.
     */
    @JvmStatic
    @JvmOverloads
    fun render(cells: PetalFrameCells, options: PetalRenderOptions = PetalRenderOptions.DEFAULT): PetalRgb =
        renderRows(cells, options, cardHeightPx(options.size), PetalLayout.SQUARE_TOP)

    /** Renders only the square of the card, [PetalRenderOptions.size] pixels on a side. */
    internal fun renderSquare(cells: PetalFrameCells, options: PetalRenderOptions): PetalRgb =
        renderRows(cells, options, options.size, 0.0)

    /** Renders [height] pixel rows of the card, the first [top] canvas units above the square's top edge. */
    private fun renderRows(cells: PetalFrameCells, options: PetalRenderOptions, height: Int, top: Double): PetalRgb {
        val size = options.size
        val s = options.supersample
        val palette = options.palette
        val petals = cells.petalView
        // the axis of every lit petal: its outward direction plus its turn
        val turns = cells.turnView
        val sin = DoubleArray(PetalLayout.FIELD_CELLS)
        val cos = DoubleArray(PetalLayout.FIELD_CELLS)
        for (cell in 0 until PetalLayout.FIELD_CELLS) {
            if (!petals[cell]) continue
            if (turns[cell].toInt() == 0) {
                sin[cell] = PetalLayout.CELL_SIN[cell]
                cos[cell] = PetalLayout.CELL_COS[cell]
            } else {
                val angle = PetalLayout.CELL_ANGLE[cell] + PetalTurns.turnAngle(turns[cell].toInt())
                sin[cell] = StrictMath.sin(angle)
                cos[cell] = StrictMath.cos(angle)
            }
        }
        val axes = Axes(sin, cos)
        val unit = PetalLayout.CANVAS / size.toDouble()
        val data = ByteArray(size * height * 3)
        val n = s * s
        val tile = DoubleArray(3)
        for (py in 0 until height) {
            for (px in 0 until size) {
                var r = 0
                var g = 0
                var b = 0
                for (sy in 0 until s) {
                    for (sx in 0 until s) {
                        val x = (px.toDouble() + (sx.toDouble() + 0.5) / s.toDouble()) * unit
                        val y = (py.toDouble() + (sy.toDouble() + 0.5) / s.toDouble()) * unit - top
                        val colour = if (y >= 0.0 && y < PetalLayout.CANVAS) {
                            shade(cells, petals, axes, palette, x, y)
                        } else {
                            kana(cells, palette, x, y, tile)
                        }
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
        return PetalRgb.wrap(size, height, data)
    }

    /**
     * The colour of a katakana tile at the square-coordinate point `(x, y)` outside the square, or
     * the background between tiles ([scratch] holds three values).
     */
    private fun kana(cells: PetalFrameCells, palette: PetalPalette, x: Double, y: Double, scratch: DoubleArray): Int {
        if (!PetalLayout.kanaTileAt(x, y, scratch)) return palette.background
        val tile = scratch[0].toInt()
        val u = scratch[1]
        val v = scratch[2]
        if (!PetalLayout.kanaInside(u, v)) return palette.background
        val inked = PetalLayout.inked(cells.kanaGlyphView[tile].toInt(), u, v)
        return if (cells.kanaLightView[tile]) {
            if (inked) palette.ink else palette.light
        } else {
            if (inked) palette.pink else palette.background
        }
    }

    /** Builds the vector draw list of [cells] for a native 2D canvas. */
    @JvmStatic
    @JvmOverloads
    fun drawList(cells: PetalFrameCells, palette: PetalPalette = PetalPalette.DEFAULT): PetalDrawList =
        PetalDrawList.of(cells, palette)

    /** Sine and cosine of the axis of every lit petal. */
    private class Axes(val sin: DoubleArray, val cos: DoubleArray)

    /** The colour of the lit petal that covers `(x, y)`, or `-1`. */
    private fun petal(
        petals: BooleanArray,
        axes: Axes,
        palette: PetalPalette,
        x: Double,
        y: Double,
        radius: Double,
        theta: Double,
    ): Int {
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
                val lit = PetalLayout.petalLit(x - PetalLayout.CELL_X[cell], y - PetalLayout.CELL_Y[cell], axes.sin[cell], axes.cos[cell])
                if (lit) return if (PetalLayout.CELL_LIGHT[cell]) palette.light else palette.pink
            }
        }
        return -1
    }

    private fun shade(cells: PetalFrameCells, petals: BooleanArray, axes: Axes, palette: PetalPalette, x: Double, y: Double): Int {
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
            val colour = petal(petals, axes, palette, x, y, radius, theta)
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
class PetalDrawPetal @JvmOverloads constructor(
    /** Field cell index (`0..3375`). */
    val cell: Int,
    /** Centre `x`. */
    val centerX: Double,
    /** Centre `y`. */
    val centerY: Double,
    /**
     * Axis of the petal: its outward direction plus its turn ([turn]), in radians, clockwise on
     * the screen from 3 o'clock.
     */
    val angle: Double,
    /** Whether the petal lies inside the `天` and is drawn in the light colour (pink otherwise). */
    val light: Boolean,
    /** Turn position (`0..3`) included in [angle]; `0` for a petal that carries no turn bits. */
    val turn: Int = 0,
)

/** One katakana tile of a [PetalDrawList], in square design units. */
class PetalDrawTile internal constructor(
    /** Tile index ([PetalLayout.kanaTiles] order). */
    val tile: Int,
    /** Left edge of the tile square (square coordinates). */
    val x: Double,
    /** Top edge of the tile square (square coordinates: negative above the square). */
    val y: Double,
    /** The glyph it shows (`0..15`, see [PetalGlyphs]). */
    val glyph: Int,
    /** Whether the tile is light (filled, glyph in ink) rather than dark (glyph in pink). */
    val light: Boolean,
)

/**
 * A platform-neutral vector description of one frame for native 2D canvases.
 *
 * Coordinates are square design units (`0..1024`, `y` down); the card extends
 * [SQUARE_TOP] units above and below the square, so a canvas of [CANVAS] ×
 * [CARD_HEIGHT] units translated down by [SQUARE_TOP] holds it all. Scale them to
 * the output size. Paint in this order:
 *
 * 1. fill the card with [PetalPalette.background];
 * 2. fill [finderDiscs] (centre disc and five petals per finder) with
 *    [PetalPalette.light], then [finderNotches] with the background colour;
 * 3. fill [backdropCells] with [PetalPalette.backdrop], clipped to the disc of
 *    radius [BACKDROP_RADIUS] around the canvas centre (a circular clip is fine);
 * 4. fill every one of [petals] in [petalColor], antialiased: an ellipse centred
 *    on the petal with half-length [PETAL_HALF_LENGTH] along
 *    [PetalDrawPetal.angle] (the outward direction plus the turn) and half-width
 *    [PETAL_HALF_WIDTH], minus the notch (a disc of radius [PETAL_NOTCH_RADIUS]
 *    centred [PETAL_NOTCH_DISTANCE] along the axis from the petal centre). The
 *    notch must be drawn: it tells a petal from one turned by half a turn, and
 *    the turn reader's templates include it;
 * 5. fill [dots] with [PetalPalette.pink];
 * 6. for every one of [kanaTiles]: a light tile is a square of side [KANA_TILE]
 *    with corners rounded to radius [KANA_CORNER], filled with
 *    [PetalPalette.light]; then the tile's glyph is stroked along
 *    [kanaGlyphStrokes] (polylines in tile-local units from the tile's top-left
 *    corner, round caps and joins, width [KANA_STROKE_WIDTH]) in
 *    [PetalPalette.ink] on a light tile and [PetalPalette.pink] on a dark one.
 *    Every stroke lies inside its rounded tile, so no clip is needed.
 *
 * The finder and backdrop lists, the petal, dot and tile shapes are shared
 * between frames, so a list costs three small arrays per frame.
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
    /** Every katakana tile of both bands, in tile order. */
    val kanaTiles: List<PetalDrawTile>,
) {
    /** Colour of [petal]: light inside the `天`, pink outside. */
    fun petalColor(petal: PetalDrawPetal): Int = if (petal.light) palette.light else palette.pink

    /** Colour of [tile]'s glyph: ink on a light tile, pink on a dark one. */
    fun glyphColor(tile: PetalDrawTile): Int = if (tile.light) palette.ink else palette.pink

    companion object {
        /** Canvas side in design units. */
        const val CANVAS = PetalLayout.CANVAS

        /** Height of the card in design units: the square with a band above and below it. */
        const val CARD_HEIGHT = PetalLayout.CARD_HEIGHT

        /** Distance from the card's top edge to the square's, in design units. */
        const val SQUARE_TOP = PetalLayout.SQUARE_TOP

        /** Side of a katakana tile. */
        const val KANA_TILE = PetalLayout.KANA_TILE

        /** Corner radius of a katakana tile. */
        const val KANA_CORNER = PetalLayout.KANA_CORNER

        /** Width of a glyph's strokes in design units: the design grid's stroke width scaled into the glyph box. */
        const val KANA_STROKE_WIDTH = PetalGlyphs.STROKE_WIDTH * PetalLayout.KANA_GLYPH_BOX / PetalGlyphs.GLYPH_GRID

        /**
         * The strokes of [glyph] (`0..15`) in tile-local design units: polylines `u0, v0, u1, v1, …`
         * measured from the tile's top-left corner (the glyph box is inset by 1 unit).
         */
        @JvmStatic
        fun kanaGlyphStrokes(glyph: Int): List<DoubleArray> {
            require(glyph in 0 until PetalGlyphs.GLYPH_COUNT) { "glyph index out of range" }
            val inset = (PetalLayout.KANA_TILE - PetalLayout.KANA_GLYPH_BOX) / 2.0
            val scale = PetalLayout.KANA_GLYPH_BOX / PetalGlyphs.GLYPH_GRID
            return java.util.Collections.unmodifiableList(
                PetalGlyphs.strokes(glyph).map { stroke -> DoubleArray(stroke.size) { inset + stroke[it] * scale } },
            )
        }

        /** Radius of the disc around the canvas centre that clips the backdrop. */
        const val BACKDROP_RADIUS = PetalLayout.BACKDROP_RADIUS

        /** Half the length of a petal, along its axis. */
        const val PETAL_HALF_LENGTH = PetalLayout.PETAL_HALF_LENGTH

        /** Half the width of a petal. */
        const val PETAL_HALF_WIDTH = PetalLayout.PETAL_HALF_WIDTH

        /** Radius of the notch at a petal tip. */
        const val PETAL_NOTCH_RADIUS = PetalLayout.PETAL_NOTCH_RADIUS

        /** Distance from a petal centre to its notch centre, along its axis. */
        const val PETAL_NOTCH_DISTANCE = PetalLayout.PETAL_NOTCH_DISTANCE

        private val FINDER_DISCS: List<PetalCircle>
        private val FINDER_NOTCHES: List<PetalCircle>
        private val BACKDROP_CELLS: List<PetalRect>

        /** The unturned petal of every field cell and the dot of every ring slot, shared by all lists. */
        private val CELL_PETALS: Array<PetalDrawPetal>
        private val SLOT_DOTS: Array<PetalCircle>

        /**
         * The turned petals (turn `t` of cell `c` at `3c + t − 1`), shared by all lists and made on
         * first use: they are immutable, so a racing thread at worst makes an equal one.
         */
        private val TURNED_PETALS = arrayOfNulls<PetalDrawPetal>(PetalLayout.FIELD_CELLS * (PetalTurns.TURN_POSITIONS - 1))

        /** The tiles (glyph `g`, light `l` of tile `t` at `32t + 2g + l`), shared and made on first use like [TURNED_PETALS]. */
        private val TILES = arrayOfNulls<PetalDrawTile>(PetalLayout.KANA_TILE_COUNT * PetalGlyphs.GLYPH_COUNT * 2)

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

        /** The shared petal of [cell] at turn position [turn]. */
        private fun petal(cell: Int, turn: Int): PetalDrawPetal {
            if (turn == 0) return CELL_PETALS[cell]
            val index = cell * (PetalTurns.TURN_POSITIONS - 1) + turn - 1
            return TURNED_PETALS[index] ?: PetalDrawPetal(
                cell,
                PetalLayout.CELL_X[cell],
                PetalLayout.CELL_Y[cell],
                PetalLayout.CELL_ANGLE[cell] + PetalTurns.turnAngle(turn),
                PetalLayout.CELL_LIGHT[cell],
                turn,
            ).also { TURNED_PETALS[index] = it }
        }

        /** Builds the draw list of [cells]. */
        @JvmStatic
        @JvmOverloads
        fun of(cells: PetalFrameCells, palette: PetalPalette = PetalPalette.DEFAULT): PetalDrawList {
            val lit = cells.petalView
            val turns = cells.turnView
            val petals = ArrayList<PetalDrawPetal>(PetalLayout.FIELD_CELLS / 2)
            for (cell in 0 until PetalLayout.FIELD_CELLS) if (lit[cell]) petals += petal(cell, turns[cell].toInt())
            val dots = ArrayList<PetalCircle>()
            for (flat in 0 until PetalLayout.TOTAL_SLOTS) if (cells.isDotLit(flat)) dots += SLOT_DOTS[flat]
            val glyphs = cells.kanaGlyphView
            val light = cells.kanaLightView
            val tiles = ArrayList<PetalDrawTile>(PetalLayout.KANA_TILE_COUNT)
            for (tile in 0 until PetalLayout.KANA_TILE_COUNT) {
                val glyph = glyphs[tile].toInt()
                val index = tile * 2 * PetalGlyphs.GLYPH_COUNT + 2 * glyph + (if (light[tile]) 1 else 0)
                tiles += TILES[index] ?: PetalDrawTile(tile, PetalLayout.TILE_X[tile], PetalLayout.TILE_Y[tile], glyph, light[tile])
                    .also { TILES[index] = it }
            }
            return PetalDrawList(
                palette,
                FINDER_DISCS,
                FINDER_NOTCHES,
                BACKDROP_CELLS,
                java.util.Collections.unmodifiableList(petals),
                java.util.Collections.unmodifiableList(dots),
                java.util.Collections.unmodifiableList(tiles),
            )
        }
    }
}
