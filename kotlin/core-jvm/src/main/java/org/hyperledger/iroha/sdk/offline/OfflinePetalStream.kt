package org.hyperledger.iroha.sdk.offline

import java.util.zip.CRC32

/** Petal stream framing helpers for offline payload transfer. */
object OfflinePetalStream {

    private val MAGIC = byteArrayOf(0x50, 0x53)
    private const val VERSION: Byte = 1
    private const val HEADER_LEN = 9
    private val GRID_SIZES = intArrayOf(33, 37, 41, 45, 49, 53, 57, 61, 65, 69)

    @JvmStatic
    fun gridSizes(): IntArray = GRID_SIZES.copyOf()

    class Options @JvmOverloads constructor(
        val gridSize: Int = 0,
        val border: Int = 1,
        val anchorSize: Int = 3,
    ) {
        init {
            require(gridSize in 0..0xFFFF) { "gridSize must be between 0 and 65535" }
            require(border in 1..0xFF) { "border must be between 1 and 255" }
            require(anchorSize in 1..0xFF) { "anchorSize must be between 1 and 255" }
        }
    }

    class Grid(val gridSize: Int, cells: BooleanArray) {
        private val _cells: BooleanArray

        init {
            require(gridSize > 0) { "gridSize must be > 0" }
            require(cells.size == gridSize * gridSize) { "cells length does not match grid size" }
            _cells = cells.copyOf()
        }

        fun get(x: Int, y: Int): Boolean {
            if (x < 0 || y < 0 || x >= gridSize || y >= gridSize) return false
            return _cells[y * gridSize + x]
        }

        fun cells(): BooleanArray = _cells.copyOf()
    }

    class SampleGrid(val gridSize: Int, samples: IntArray) {
        private val _samples: IntArray

        init {
            require(gridSize > 0) { "gridSize must be > 0" }
            require(samples.size == gridSize * gridSize) { "samples length does not match grid size" }
            _samples = samples.copyOf()
        }

        fun samples(): IntArray = _samples.copyOf()
    }

    object Encoder {
        @JvmStatic
        fun encodeGrid(payload: ByteArray, options: Options?): Grid {
            val normalized = options ?: Options()
            require(payload.size <= 0xFFFF) { "payload length exceeds 65535" }
            val resolved = resolveGridSize(payload.size, normalized)
            val capacity = capacityBits(resolved, normalized)
            val bitsNeeded = (HEADER_LEN + payload.size) * 8
            require(bitsNeeded <= capacity) { "petal stream grid too small for payload" }
            val header = encodeHeader(payload)
            val bits = ArrayList<Boolean>(bitsNeeded)
            pushBytesAsBits(header, bits)
            pushBytesAsBits(payload, bits)
            val cells = BooleanArray(resolved * resolved)
            var bitIndex = 0
            for (y in 0 until resolved) {
                for (x in 0 until resolved) {
                    val idx = y * resolved + x
                    val role = cellRole(x, y, resolved, normalized)
                    when (role) {
                        CellRole.BORDER, CellRole.ANCHOR_DARK -> cells[idx] = true
                        CellRole.ANCHOR_LIGHT -> cells[idx] = false
                        CellRole.DATA -> {
                            if (bitIndex < bits.size) {
                                cells[idx] = bits[bitIndex]
                                bitIndex += 1
                            }
                        }
                    }
                }
            }
            return Grid(resolved, cells)
        }

        @JvmStatic
        fun encodeGrids(payloads: List<ByteArray?>, options: Options?): EncodeResult {
            val normalized = options ?: Options()
            var maxLen = 0
            for (p in payloads) {
                if (p != null && p.size > maxLen) maxLen = p.size
            }
            val resolved = if (normalized.gridSize == 0)
                resolveGridSize(maxLen, normalized) else normalized.gridSize
            val fixed = Options(resolved, normalized.border, normalized.anchorSize)
            val grids = payloads.map { encodeGrid(it ?: byteArrayOf(), fixed) }
            return EncodeResult(resolved, grids)
        }
    }

    class EncodeResult(val gridSize: Int, val grids: List<Grid>)

    object Decoder {
        @JvmStatic
        fun decodeGrid(grid: Grid, options: Options?): ByteArray {
            val normalized = options ?: Options()
            val resolved = resolveGridSizeForDecode(grid.gridSize, normalized)
            require(resolved == grid.gridSize) { "grid size mismatch" }
            val bits = ArrayList<Boolean>()
            for (y in 0 until resolved) {
                for (x in 0 until resolved) {
                    if (cellRole(x, y, resolved, normalized) == CellRole.DATA) {
                        bits.add(grid.get(x, y))
                    }
                }
            }
            val bytes = bitsToBytes(bits)
            return decodePayload(bytes)
        }

        @JvmStatic
        fun decodeSamples(samples: SampleGrid, options: Options?): ByteArray {
            val normalized = options ?: Options()
            val resolved = resolveGridSizeForDecode(samples.gridSize, normalized)
            require(resolved == samples.gridSize) { "sample grid size mismatch" }
            var darkSum = 0L
            var lightSum = 0L
            var darkCount = 0L
            var lightCount = 0L
            val values = samples.samples()
            for (y in 0 until resolved) {
                for (x in 0 until resolved) {
                    val idx = y * resolved + x
                    val value = values[idx]
                    when (cellRole(x, y, resolved, normalized)) {
                        CellRole.ANCHOR_DARK -> { darkSum += value; darkCount++ }
                        CellRole.ANCHOR_LIGHT -> { lightSum += value; lightCount++ }
                        else -> {}
                    }
                }
            }
            require(darkCount != 0L && lightCount != 0L) { "anchor sampling failed" }
            val darkAvg = darkSum.toDouble() / darkCount.toDouble()
            val lightAvg = lightSum.toDouble() / lightCount.toDouble()
            require(darkAvg < lightAvg) { "anchor contrast too low" }
            val threshold = Math.round((darkAvg + lightAvg) / 2.0).toInt()
            val cells = BooleanArray(values.size) { values[it] < threshold }
            return decodeGrid(Grid(resolved, cells), normalized)
        }
    }

    object Sampler {
        @JvmStatic
        fun sampleGridFromRgba(
            rgba: ByteArray, width: Int, height: Int, gridSize: Int,
        ): SampleGrid {
            require(width > 0 && height > 0) { "image dimensions must be positive" }
            require(gridSize > 0) { "gridSize must be > 0" }
            val size = minOf(width, height)
            val offsetX = (width - size) / 2
            val offsetY = (height - size) / 2
            val cellSize = maxOf(1, size / gridSize)
            val samples = IntArray(gridSize * gridSize)
            var out = 0
            for (y in 0 until gridSize) {
                for (x in 0 until gridSize) {
                    var sum = 0L; var count = 0L
                    for (oy in doubleArrayOf(0.25, 0.5, 0.75)) {
                        for (ox in doubleArrayOf(0.25, 0.5, 0.75)) {
                            val px = minOf(width - 1, offsetX + Math.floor((x + ox) * cellSize).toInt())
                            val py = minOf(height - 1, offsetY + Math.floor((y + oy) * cellSize).toInt())
                            val idx = (py * width + px) * 4
                            if (idx + 2 >= rgba.size) continue
                            val r = rgba[idx].toInt() and 0xFF
                            val g = rgba[idx + 1].toInt() and 0xFF
                            val b = rgba[idx + 2].toInt() and 0xFF
                            val luma = (77 * r + 150 * g + 29 * b) shr 8
                            sum += luma; count++
                        }
                    }
                    samples[out++] = if (count == 0L) 0 else (sum / count).toInt()
                }
            }
            return SampleGrid(gridSize, samples)
        }

        @JvmStatic
        fun sampleGridFromLuma(
            luma: ByteArray, width: Int, height: Int, gridSize: Int,
        ): SampleGrid {
            require(width > 0 && height > 0) { "image dimensions must be positive" }
            require(gridSize > 0) { "gridSize must be > 0" }
            val size = minOf(width, height)
            val offsetX = (width - size) / 2
            val offsetY = (height - size) / 2
            val cellSize = maxOf(1, size / gridSize)
            val samples = IntArray(gridSize * gridSize)
            var out = 0
            for (y in 0 until gridSize) {
                for (x in 0 until gridSize) {
                    var sum = 0L; var count = 0L
                    for (oy in doubleArrayOf(0.25, 0.5, 0.75)) {
                        for (ox in doubleArrayOf(0.25, 0.5, 0.75)) {
                            val px = minOf(width - 1, offsetX + Math.floor((x + ox) * cellSize).toInt())
                            val py = minOf(height - 1, offsetY + Math.floor((y + oy) * cellSize).toInt())
                            val idx = py * width + px
                            if (idx >= luma.size) continue
                            sum += luma[idx].toInt() and 0xFF; count++
                        }
                    }
                    samples[out++] = if (count == 0L) 0 else (sum / count).toInt()
                }
            }
            return SampleGrid(gridSize, samples)
        }
    }

    private enum class CellRole { BORDER, ANCHOR_DARK, ANCHOR_LIGHT, DATA }

    private fun resolveGridSize(payloadLength: Int, options: Options): Int {
        require(options.border > 0) { "border must be > 0" }
        require(options.anchorSize > 0) { "anchorSize must be > 0" }
        val bitsNeeded = (HEADER_LEN + payloadLength) * 8
        if (options.gridSize != 0) {
            val capacity = capacityBits(options.gridSize, options)
            require(bitsNeeded <= capacity) { "petal stream grid too small for payload" }
            return options.gridSize
        }
        for (candidate in GRID_SIZES) {
            if (bitsNeeded <= capacityBits(candidate, options)) return candidate
        }
        throw IllegalArgumentException("petal stream grid too small for payload")
    }

    private fun resolveGridSizeForDecode(gridSize: Int, options: Options): Int {
        if (options.gridSize != 0) return options.gridSize
        require(gridSize != 0) { "grid size is zero" }
        return gridSize
    }

    private fun capacityBits(gridSize: Int, options: Options): Int {
        require(gridSize > 0) { "grid size must be > 0" }
        val minGrid = options.border * 2 + options.anchorSize * 2 + 1
        require(gridSize >= minGrid) { "grid size too small for anchors" }
        val total = gridSize * gridSize
        val borderCells = gridSize * 4 - 4
        val anchorCells = options.anchorSize * options.anchorSize * 4
        return maxOf(0, total - borderCells - anchorCells)
    }

    private fun cellRole(x: Int, y: Int, gridSize: Int, options: Options): CellRole {
        val border = options.border
        val anchor = options.anchorSize
        if (x < border || y < border || x >= gridSize - border || y >= gridSize - border) {
            return CellRole.BORDER
        }
        val right = gridSize - border - anchor
        val bottom = gridSize - border - anchor
        val inLeft = x in border until border + anchor
        val inRight = x in right until right + anchor
        val inTop = y in border until border + anchor
        val inBottom = y in bottom until bottom + anchor
        if (inLeft && inTop) return CellRole.ANCHOR_DARK
        if (inLeft && inBottom) return CellRole.ANCHOR_DARK
        if (inRight && inTop) return CellRole.ANCHOR_LIGHT
        if (inRight && inBottom) return CellRole.ANCHOR_LIGHT
        return CellRole.DATA
    }

    private fun encodeHeader(payload: ByteArray): ByteArray {
        val out = ByteArray(HEADER_LEN)
        System.arraycopy(MAGIC, 0, out, 0, MAGIC.size)
        out[2] = VERSION
        writeUInt16LE(out, 3, payload.size)
        val crc32 = CRC32()
        crc32.update(payload)
        writeUInt32LE(out, 5, crc32.value)
        return out
    }

    private fun decodePayload(bytes: ByteArray): ByteArray {
        require(bytes.size >= HEADER_LEN) { "petal stream header too short" }
        require(bytes[0] == MAGIC[0] && bytes[1] == MAGIC[1]) { "petal stream magic mismatch" }
        require(bytes[2] == VERSION) { "petal stream version mismatch" }
        val payloadLength = readUInt16LE(bytes, 3)
        val expected = readUInt32LE(bytes, 5)
        val start = HEADER_LEN
        val end = start + payloadLength
        require(end <= bytes.size) { "petal stream payload length exceeds data" }
        val payload = bytes.copyOfRange(start, end)
        val crc32 = CRC32()
        crc32.update(payload)
        require(crc32.value == expected) { "petal stream checksum mismatch" }
        return payload
    }

    private fun pushBytesAsBits(bytes: ByteArray, out: MutableList<Boolean>) {
        for (value in bytes) {
            for (bit in 7 downTo 0) {
                out.add((value.toInt() and (1 shl bit)) != 0)
            }
        }
    }

    private fun bitsToBytes(bits: List<Boolean>): ByteArray {
        val out = ByteArray((bits.size + 7) / 8)
        var offset = 0
        var index = 0
        while (offset < bits.size) {
            var value = 0
            for (bit in 0 until 8) {
                if (offset + bit < bits.size && bits[offset + bit]) {
                    value = value or (1 shl (7 - bit))
                }
            }
            out[index++] = value.toByte()
            offset += 8
        }
        return out
    }

    private fun writeUInt16LE(out: ByteArray, offset: Int, value: Int) {
        out[offset] = (value and 0xFF).toByte()
        out[offset + 1] = ((value shr 8) and 0xFF).toByte()
    }

    private fun writeUInt32LE(out: ByteArray, offset: Int, value: Long) {
        out[offset] = (value and 0xFF).toByte()
        out[offset + 1] = ((value shr 8) and 0xFF).toByte()
        out[offset + 2] = ((value shr 16) and 0xFF).toByte()
        out[offset + 3] = ((value shr 24) and 0xFF).toByte()
    }

    private fun readUInt16LE(bytes: ByteArray, offset: Int): Int =
        (bytes[offset].toInt() and 0xFF) or ((bytes[offset + 1].toInt() and 0xFF) shl 8)

    private fun readUInt32LE(bytes: ByteArray, offset: Int): Long {
        var value = 0L
        for (i in 0 until 4) {
            value = value or ((bytes[offset + i].toLong() and 0xFF) shl (i * 8))
        }
        return value
    }
}
