package org.hyperledger.iroha.sdk.offline.petal

/**
 * Scratch buffers reused across decodes so a scanning session does not
 * allocate several megabytes per camera frame. Buffers grow on demand and are
 * never shrunk; the field buffers are allocated on first use. Not thread-safe:
 * one workspace per decoding thread.
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

    private var reads: Array<PetalDecoder.FieldRead>? = null
    private var local: DoubleArray? = null
    private var mean: DoubleArray? = null

    /**
     * Two field reads with their level buffers: the best pose so far and the one being read, which
     * the distortion search swaps.
     */
    val fieldReads: Array<PetalDecoder.FieldRead>
        get() = reads ?: Array(2) { PetalDecoder.FieldRead(DoubleArray(PetalLayout.FIELD_CELLS)) }.also { reads = it }

    /** Local levels of the field cells (the local read). */
    val localLevels: DoubleArray
        get() = local ?: DoubleArray(PetalLayout.FIELD_CELLS).also { local = it }

    /** Cell-by-cell mean of merged field evidence. */
    val meanLevels: DoubleArray
        get() = mean ?: DoubleArray(PetalLayout.FIELD_CELLS).also { mean = it }

    private var kana: PetalKanaReader? = null

    /** The kana reader and its scratch buffers (a few kilobytes). */
    val kanaReader: PetalKanaReader
        get() = kana ?: PetalKanaReader().also { kana = it }

    private var turns: PetalTurnReader? = null

    /** The turn reader and its carrier windows (about 2 MB once a capture has turn lanes to read). */
    val turnReader: PetalTurnReader
        get() = turns ?: PetalTurnReader().also { turns = it }

    /** The levels of the bit cells of the sector being read, in bit order. */
    val sectorValues = DoubleArray(PetalLanes.FIELD_BITS)

    /** The levels of one colour class of the sector being read. */
    val classValues = DoubleArray(PetalLanes.FIELD_BITS)

    /** The transmitted word of the sector being read and the confidence of each byte. */
    val wordBytes = ByteArray(PetalLanes.FIELD_WORD)
    val wordConfidence = DoubleArray(PetalLanes.FIELD_WORD)

    private var neighbours = DoubleArray(0)

    /** Sorting scratch for the levels of one neighbourhood of [size] cells. */
    fun neighbourValues(size: Int): DoubleArray {
        if (neighbours.size < size) neighbours = DoubleArray(size)
        return neighbours
    }

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
}
