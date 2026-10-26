package org.hyperledger.iroha.sdk.offline.petal

/**
 * The sixteen katakana of the band tiles (see [PetalKana]).
 *
 * A band tile's glyph is a four-bit symbol. The alphabet is the subset of the Iroha ordering
 * `イロハニホヘト…ス` whose members stay most distinguishable after camera blur:
 * イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol `0` is `イ` and symbol `15` is `ン`.
 *
 * Glyphs are defined as stroke polylines on a 32 × 32 grid (`y` grows downward) drawn with round
 * caps and joins of width [STROKE_WIDTH]. A point is inked when it lies within half a stroke width
 * of a stroke segment ([isInked]); renderers and the band reader's templates both follow that
 * definition, so classification never depends on a rasteriser.
 */
object PetalGlyphs {
    /** Number of glyphs (symbols) in the alphabet. */
    const val GLYPH_COUNT = 16

    /** Side of the glyph design grid. */
    const val GLYPH_GRID = 32.0

    /** Stroke width on the design grid. */
    const val STROKE_WIDTH = 6.5

    /** The sixteen characters in symbol order. */
    const val GLYPH_CHARS = "イロハニヘトワカレムノケアヒスン"

    /**
     * Stroke definitions for every glyph, in symbol order: each glyph is a list of strokes and each
     * stroke a polyline of `x0, y0, x1, y1, …` points on the design grid.
     */
    private val STROKES: Array<Array<DoubleArray>> = arrayOf(
        // イ
        arrayOf(doubleArrayOf(22.0, 4.0, 8.0, 18.0), doubleArrayOf(19.0, 10.0, 19.0, 29.0)),
        // ロ
        arrayOf(doubleArrayOf(6.0, 6.0, 26.0, 6.0, 26.0, 26.0, 6.0, 26.0, 6.0, 6.0)),
        // ハ
        arrayOf(doubleArrayOf(14.0, 6.0, 6.0, 27.0), doubleArrayOf(18.0, 10.0, 27.0, 27.0)),
        // ニ
        arrayOf(doubleArrayOf(8.0, 10.0, 24.0, 10.0), doubleArrayOf(4.0, 24.0, 28.0, 24.0)),
        // ヘ
        arrayOf(doubleArrayOf(3.0, 19.0, 12.0, 10.0, 29.0, 24.0)),
        // ト
        arrayOf(doubleArrayOf(11.0, 3.0, 11.0, 29.0), doubleArrayOf(11.0, 13.0, 26.0, 21.0)),
        // ワ
        arrayOf(doubleArrayOf(7.0, 21.0, 7.0, 8.0, 25.0, 8.0, 23.0, 19.0, 10.0, 29.0)),
        // カ
        arrayOf(
            doubleArrayOf(14.0, 3.0, 14.0, 16.0, 8.0, 28.0),
            doubleArrayOf(4.0, 12.0, 25.0, 12.0, 25.0, 23.0, 21.0, 28.0),
        ),
        // レ
        arrayOf(doubleArrayOf(9.0, 4.0, 9.0, 27.0, 27.0, 8.0)),
        // ム
        arrayOf(doubleArrayOf(17.0, 4.0, 7.0, 23.0, 27.0, 24.0), doubleArrayOf(21.0, 16.0, 26.0, 22.0)),
        // ノ
        arrayOf(doubleArrayOf(22.0, 4.0, 17.0, 16.0, 9.0, 28.0)),
        // ケ
        arrayOf(
            doubleArrayOf(13.0, 3.0, 6.0, 13.0),
            doubleArrayOf(10.0, 12.0, 27.0, 12.0),
            doubleArrayOf(19.0, 12.0, 16.0, 22.0, 9.0, 29.0),
        ),
        // ア
        arrayOf(doubleArrayOf(5.0, 10.0, 26.0, 10.0, 25.0, 18.0, 19.0, 24.0), doubleArrayOf(15.0, 10.0, 15.0, 21.0, 8.0, 29.0)),
        // ヒ
        arrayOf(doubleArrayOf(10.0, 5.0, 10.0, 26.0, 26.0, 26.0), doubleArrayOf(10.0, 14.0, 25.0, 14.0)),
        // ス
        arrayOf(doubleArrayOf(6.0, 5.0, 24.0, 5.0, 17.0, 15.0, 6.0, 27.0), doubleArrayOf(13.0, 15.0, 28.0, 28.0)),
        // ン
        arrayOf(doubleArrayOf(6.0, 7.0, 12.0, 13.0), doubleArrayOf(6.0, 25.0, 14.0, 27.0, 27.0, 9.0)),
    )

    init {
        check(STROKES.size == GLYPH_COUNT && GLYPH_CHARS.length == GLYPH_COUNT) { "sixteen glyphs" }
    }

    /** The strokes of [glyph] (`0..15`), each a polyline `x0, y0, x1, y1, …` on the design grid (copies). */
    @JvmStatic
    fun strokes(glyph: Int): List<DoubleArray> {
        require(glyph in 0 until GLYPH_COUNT) { "glyph index out of range" }
        return java.util.Collections.unmodifiableList(STROKES[glyph].map { it.copyOf() })
    }

    /** Whether the design-grid point `(x, y)` is inked in [glyph] (`0..15`). */
    @JvmStatic
    fun isInked(glyph: Int, x: Double, y: Double): Boolean {
        require(glyph in 0 until GLYPH_COUNT) { "glyph index out of range" }
        return inked(glyph, x, y)
    }

    /** [isInked] without the range check. */
    internal fun inked(glyph: Int, x: Double, y: Double): Boolean {
        val radius = STROKE_WIDTH / 2.0
        for (stroke in STROKES[glyph]) {
            var at = 0
            while (at + 3 < stroke.size) {
                if (segmentDistance(x, y, stroke[at], stroke[at + 1], stroke[at + 2], stroke[at + 3]) <= radius) return true
                at += 2
            }
        }
        return false
    }

    /** Distance from point `(px, py)` to the segment `(ax, ay)`–`(bx, by)`. */
    private fun segmentDistance(px: Double, py: Double, ax: Double, ay: Double, bx: Double, by: Double): Double {
        val dx = bx - ax
        val dy = by - ay
        val lengthSq = dx * dx + dy * dy
        val t = if (lengthSq == 0.0) {
            0.0
        } else {
            (((px - ax) * dx + (py - ay) * dy) / lengthSq).coerceIn(0.0, 1.0)
        }
        val ex = px - (ax + t * dx)
        val ey = py - (ay + t * dy)
        return Math.sqrt(ex * ex + ey * ey)
    }
}
