package org.hyperledger.iroha.sdk.offline.petal

/**
 * Scratch buffers reused across decodes so a scanning session does not
 * allocate several megabytes per camera frame. Buffers grow on demand and are
 * never shrunk. Not thread-safe: one workspace per decoding thread.
 */
internal class PetalWorkspace {
    var integral = IntArray(0)
        private set
    var mask = BooleanArray(0)
        private set
    var labels = IntArray(0)
        private set
    var parent = IntArray(1024)
        private set

    // Connected components, structure of arrays indexed by root label, then compacted.
    var area = IntArray(0)
        private set
    var minX = IntArray(0)
        private set
    var maxX = IntArray(0)
        private set
    var minY = IntArray(0)
        private set
    var maxY = IntArray(0)
        private set
    var sumX = DoubleArray(0)
        private set
    var sumY = DoubleArray(0)
        private set
    var sumXX = DoubleArray(0)
        private set
    var sumYY = DoubleArray(0)
        private set
    var sumXY = DoubleArray(0)
        private set
    var centroidX = DoubleArray(0)
        private set
    var centroidY = DoubleArray(0)
        private set

    /** Raw tile patches in camera luma levels, as captured: `TILE_COUNT × 64` values. */
    val patches = DoubleArray(PetalLayout.TILE_COUNT * CELLS)

    /**
     * The patches as the tile read in progress compares them with its
     * templates (levelled against the finders, or rescaled by their own
     * contrast): `TILE_COUNT × 64` values. [patches] stays untouched so the
     * normalised read can follow a level read that did not decode.
     */
    val scaled = DoubleArray(PetalLayout.TILE_COUNT * CELLS)

    /** Matching errors of the 32 polarity × glyph hypotheses of one tile. */
    val errors = DoubleArray(2 * PetalGlyphs.GLYPH_COUNT)

    /** Tiles the tile read in progress gives up on (zero margins); all `false` for the level read. */
    val erased = BooleanArray(PetalLayout.TILE_COUNT)

    /** Contrast span (robust brightest minus darkest level) of every patch in the normalised read. */
    val spans = DoubleArray(PetalLayout.TILE_COUNT)

    /** Sorting scratch for the median of [spans]. */
    val sortedSpans = DoubleArray(PetalLayout.TILE_COUNT)

    /** Sorting scratch for the cells of one patch. */
    val sortedCells = DoubleArray(CELLS)

    /** The robust darkest and brightest level of the patch just analysed: `[low, high]`. */
    val levels = DoubleArray(2)

    fun ensurePixels(width: Int, height: Int) {
        val pixels = width * height
        val integralSize = (width + 1) * (height + 1)
        if (integral.size < integralSize) integral = IntArray(integralSize)
        if (mask.size < pixels) mask = BooleanArray(pixels)
        if (labels.size < pixels) labels = IntArray(pixels)
    }

    fun growParent(required: Int) {
        if (parent.size >= required) return
        var size = parent.size
        while (size < required) size = if (size > Int.MAX_VALUE / 2) Int.MAX_VALUE else size * 2
        parent = parent.copyOf(size)
    }

    fun ensureComponents(count: Int) {
        if (area.size >= count) return
        area = IntArray(count)
        minX = IntArray(count)
        maxX = IntArray(count)
        minY = IntArray(count)
        maxY = IntArray(count)
        sumX = DoubleArray(count)
        sumY = DoubleArray(count)
        sumXX = DoubleArray(count)
        sumYY = DoubleArray(count)
        sumXY = DoubleArray(count)
        centroidX = DoubleArray(count)
        centroidY = DoubleArray(count)
    }

    companion object {
        /** Cells of a tile sample patch (`8 × 8`). */
        const val CELLS = PetalGlyphs.TEMPLATE_N * PetalGlyphs.TEMPLATE_N
    }
}
