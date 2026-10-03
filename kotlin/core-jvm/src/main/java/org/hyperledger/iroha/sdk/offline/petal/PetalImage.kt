package org.hyperledger.iroha.sdk.offline.petal

import java.nio.ByteBuffer

/**
 * A single-channel 8-bit image (a camera luma plane), row-major.
 *
 * Pixel `(i, j)` covers `[i, i+1) × [j, j+1)` and its centre is at
 * `(i + 0.5, j + 0.5)`; every Petal homography maps into these pixel-edge
 * coordinates. Immutable: pixel data is copied on construction and access.
 */
class PetalLuma private constructor(
    /** Width in pixels. */
    val width: Int,
    /** Height in pixels. */
    val height: Int,
    /** Row-major samples, `width * height` bytes, owned by this image. */
    internal val pixels: ByteArray,
) {
    /** Creates a black image of the given size. */
    constructor(width: Int, height: Int) : this(width, height, ByteArray(checkedArea(width, height)))

    /** Reads pixel `(x, y)` as `0..255`. */
    fun at(x: Int, y: Int): Int {
        require(x in 0 until width && y in 0 until height) { "pixel out of range" }
        return pixels[y * width + x].toInt() and 0xFF
    }

    /** The row-major samples (a copy). */
    fun data(): ByteArray = pixels.copyOf()

    /**
     * Bilinear sample at continuous pixel-edge coordinates, clamped to the
     * image border. The image must not be empty.
     */
    fun sample(x: Double, y: Double): Double {
        check(width > 0 && height > 0) { "cannot sample an empty image" }
        val maxX = (width - 1).toDouble()
        val maxY = (height - 1).toDouble()
        var fx = x - 0.5
        if (fx < 0.0) fx = 0.0
        if (fx > maxX) fx = maxX
        var fy = y - 0.5
        if (fy < 0.0) fy = 0.0
        if (fy > maxY) fy = maxY
        val x0 = Math.floor(fx).toInt()
        val y0 = Math.floor(fy).toInt()
        val x1 = if (x0 + 1 < width) x0 + 1 else width - 1
        val y1 = if (y0 + 1 < height) y0 + 1 else height - 1
        val tx = fx - x0.toDouble()
        val ty = fy - y0.toDouble()
        val row0 = y0 * width
        val row1 = y1 * width
        val p00 = (pixels[row0 + x0].toInt() and 0xFF).toDouble()
        val p10 = (pixels[row0 + x1].toInt() and 0xFF).toDouble()
        val p01 = (pixels[row1 + x0].toInt() and 0xFF).toDouble()
        val p11 = (pixels[row1 + x1].toInt() and 0xFF).toDouble()
        val top = p00 * (1.0 - tx) + p10 * tx
        val bottom = p01 * (1.0 - tx) + p11 * tx
        return top * (1.0 - ty) + bottom * ty
    }

    override fun equals(other: Any?): Boolean = other is PetalLuma && width == other.width &&
        height == other.height && pixels.contentEquals(other.pixels)

    override fun hashCode(): Int = 31 * (31 * width + height) + pixels.contentHashCode()

    override fun toString(): String = "PetalLuma(${width}x$height)"

    companion object {
        /** Copies a `width * height` row-major buffer; `null` on a size mismatch. */
        @JvmStatic
        fun fromRaw(width: Int, height: Int, data: ByteArray): PetalLuma? {
            if (width < 0 || height < 0 || width.toLong() * height.toLong() != data.size.toLong()) return null
            return PetalLuma(width, height, data.copyOf())
        }

        /**
         * Copies a strided plane, for example the first `width * height` bytes
         * (the Y plane) of an NV21 camera frame with `stride == width`.
         * Returns `null` when the stride or plane length is inconsistent.
         */
        @JvmStatic
        fun fromStrided(width: Int, height: Int, stride: Int, plane: ByteArray): PetalLuma? {
            if (width < 0 || height <= 0 || stride < width) return null
            if (plane.size.toLong() < stride.toLong() * (height - 1).toLong() + width.toLong()) return null
            val pixels = ByteArray(checkedArea(width, height))
            for (row in 0 until height) System.arraycopy(plane, row * stride, pixels, row * width, width)
            return PetalLuma(width, height, pixels)
        }

        /**
         * Copies an 8-bit luma plane held in a [ByteBuffer] (for example plane 0
         * of a `YUV_420_888` camera image) and rotates it clockwise by
         * [rotationDegrees] (`0`, `90`, `180` or `270`).
         *
         * Sample `(x, y)` of the unrotated plane is read from
         * `buffer.position() + y * rowStride + x * pixelStride`. The buffer's
         * position and limit are not modified. Petal decoding is rotation
         * invariant, so pass `0` unless an upright image is needed elsewhere.
         *
         * @throws IllegalArgumentException when the geometry does not fit the buffer.
         */
        @JvmStatic
        @JvmOverloads
        fun fromPlane(
            buffer: ByteBuffer,
            width: Int,
            height: Int,
            rowStride: Int,
            pixelStride: Int = 1,
            rotationDegrees: Int = 0,
        ): PetalLuma {
            require(width > 0 && height > 0) { "plane must not be empty" }
            require(pixelStride >= 1 && rowStride >= (width - 1).toLong() * pixelStride + 1) { "plane strides are inconsistent" }
            require(rotationDegrees == 0 || rotationDegrees == 90 || rotationDegrees == 180 || rotationDegrees == 270) {
                "rotation must be 0, 90, 180 or 270 degrees"
            }
            val base = buffer.position()
            val needed = rowStride.toLong() * (height - 1) + (width - 1).toLong() * pixelStride + 1
            require(needed <= (buffer.limit() - base).toLong()) { "plane does not fit the buffer" }
            val area = checkedArea(width, height)
            val upright = ByteArray(area)
            val view = buffer.duplicate()
            if (pixelStride == 1) {
                for (row in 0 until height) {
                    view.position(base + row * rowStride)
                    view.get(upright, row * width, width)
                }
            } else {
                for (row in 0 until height) {
                    val start = base + row * rowStride
                    val target = row * width
                    for (column in 0 until width) upright[target + column] = view.get(start + column * pixelStride)
                }
            }
            if (rotationDegrees == 0) return PetalLuma(width, height, upright)
            val rotatedWidth = if (rotationDegrees == 180) width else height
            val rotatedHeight = if (rotationDegrees == 180) height else width
            val rotated = ByteArray(area)
            for (y in 0 until height) {
                val source = y * width
                for (x in 0 until width) {
                    val target = when (rotationDegrees) {
                        90 -> x * rotatedWidth + (height - 1 - y)
                        180 -> (height - 1 - y) * rotatedWidth + (width - 1 - x)
                        else -> (width - 1 - x) * rotatedWidth + y
                    }
                    rotated[target] = upright[source + x]
                }
            }
            return PetalLuma(rotatedWidth, rotatedHeight, rotated)
        }

        /** Wraps [pixels] without copying; the caller hands over ownership. */
        internal fun wrap(width: Int, height: Int, pixels: ByteArray): PetalLuma {
            require(width.toLong() * height.toLong() == pixels.size.toLong()) { "luma size mismatch" }
            return PetalLuma(width, height, pixels)
        }

        private fun checkedArea(width: Int, height: Int): Int {
            require(width >= 0 && height >= 0) { "image size must not be negative" }
            val area = width.toLong() * height.toLong()
            require(area <= Int.MAX_VALUE) { "image is too large" }
            return area.toInt()
        }
    }
}

/** An interleaved 8-bit RGB image, row-major `r, g, b` triples. Immutable. */
class PetalRgb private constructor(
    /** Width in pixels. */
    val width: Int,
    /** Height in pixels. */
    val height: Int,
    internal val pixels: ByteArray,
) {
    /** The interleaved samples (a copy). */
    fun data(): ByteArray = pixels.copyOf()

    /** Packed `0xRRGGBB` colour of pixel `(x, y)`. */
    fun rgbAt(x: Int, y: Int): Int {
        require(x in 0 until width && y in 0 until height) { "pixel out of range" }
        val at = (y * width + x) * 3
        return ((pixels[at].toInt() and 0xFF) shl 16) or ((pixels[at + 1].toInt() and 0xFF) shl 8) or
            (pixels[at + 2].toInt() and 0xFF)
    }

    /** Rec. 601 luma of the image. */
    fun toLuma(): PetalLuma {
        val luma = ByteArray(width * height)
        for (index in luma.indices) {
            val r = pixels[3 * index].toInt() and 0xFF
            val g = pixels[3 * index + 1].toInt() and 0xFF
            val b = pixels[3 * index + 2].toInt() and 0xFF
            luma[index] = ((299 * r + 587 * g + 114 * b + 500) / 1000).toByte()
        }
        return PetalLuma.wrap(width, height, luma)
    }

    override fun equals(other: Any?): Boolean = other is PetalRgb && width == other.width &&
        height == other.height && pixels.contentEquals(other.pixels)

    override fun hashCode(): Int = 31 * (31 * width + height) + pixels.contentHashCode()

    override fun toString(): String = "PetalRgb(${width}x$height)"

    companion object {
        /** Copies a `width * height * 3` interleaved buffer; `null` on a size mismatch. */
        @JvmStatic
        fun fromRaw(width: Int, height: Int, data: ByteArray): PetalRgb? {
            if (width < 0 || height < 0 || width.toLong() * height.toLong() * 3 != data.size.toLong()) return null
            return PetalRgb(width, height, data.copyOf())
        }

        /** Wraps [pixels] without copying; the caller hands over ownership. */
        internal fun wrap(width: Int, height: Int, pixels: ByteArray): PetalRgb = PetalRgb(width, height, pixels)
    }
}
