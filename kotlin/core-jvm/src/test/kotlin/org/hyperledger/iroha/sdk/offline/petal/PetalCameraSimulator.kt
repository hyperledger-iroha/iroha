package org.hyperledger.iroha.sdk.offline.petal

/**
 * Camera and scene parameters of the test-only capture simulator (a port of
 * the reference `sim` module used by its qualification and session tests).
 */
internal data class PetalCaptureConfig(
    val width: Int,
    val height: Int,
    /** Canvas side as a fraction of the short image side. */
    val fill: Double,
    val rotationDeg: Double,
    val tiltXDeg: Double,
    val tiltYDeg: Double,
    val shiftX: Double,
    val shiftY: Double,
    /** Radial lens distortion coefficient (negative is barrel). */
    val lensK1: Double,
    val blurSigma: Double,
    val motionPx: Double,
    val motionDeg: Double,
    val bloom: Double,
    val bloomSigma: Double,
    val exposure: Double,
    val ambient: Double,
    val gradient: Double,
    val gradientDeg: Double,
    val vignette: Double,
    val glare: Double,
    val glareAtX: Double,
    val glareAtY: Double,
    val glareSigma: Double,
    val noise: Double,
    val sharpen: Double,
    val seed: Long,
) {
    companion object {
        /** A recent phone in good light: sharp and quiet. */
        fun modern() = PetalCaptureConfig(
            1280, 720, 0.85, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.7, 0.0, 0.0, 0.03, 4.0, 1.0, 0.02, 0.05, 30.0,
            0.1, 0.0, 0.5, 0.5, 40.0, 2.5, 0.0, 1,
        )

        /** An older phone: soft focus, noisy, some tilt and barrel distortion. */
        fun legacy() = PetalCaptureConfig(
            1280, 720, 0.8, 12.0, 12.0, -10.0, 0.02, -0.01, -0.06, 1.3, 0.0, 0.0, 0.08, 5.0, 1.1, 0.06, 0.15,
            120.0, 0.25, 0.0, 0.3, 0.3, 50.0, 6.0, 0.4, 2,
        )
    }
}

/** Deterministic camera-capture simulator (perspective, lens, blur, exposure, noise, quantisation). */
internal object PetalCameraSimulator {
    private const val SS = 3

    /** SplitMix64 with a Box–Muller Gaussian. */
    private class Rng(seed: Long) {
        private var state = seed xor -0x61c8_8646_80b5_83ebL
        private var spare = Double.NaN

        fun nextLong(): Long {
            state += -0x61c8_8646_80b5_83ebL
            var z = state
            z = (z xor (z ushr 30)) * -0x40a7_b892_e31b_1a47L
            z = (z xor (z ushr 27)) * -0x6b2f_b644_ecce_ee15L
            return z xor (z ushr 31)
        }

        fun uniform(): Double = ((nextLong() ushr 11).toDouble() + 0.5) / (1L shl 53).toDouble()

        fun gaussian(): Double {
            if (!spare.isNaN()) return spare.also { spare = Double.NaN }
            val u1 = uniform()
            val u2 = uniform()
            val radius = Math.sqrt(-2.0 * Math.log(u1))
            val angle = PetalLayout.TAU * u2
            spare = radius * Math.sin(angle)
            return radius * Math.cos(angle)
        }
    }

    private fun gaussianKernel(sigma: Double): DoubleArray {
        val radius = Math.max(Math.ceil(3.0 * sigma), 1.0).toInt()
        val kernel = DoubleArray(2 * radius + 1) { val i = (it - radius).toDouble(); Math.exp(-(i * i) / (2.0 * sigma * sigma)) }
        val sum = kernel.sum()
        for (index in kernel.indices) kernel[index] /= sum
        return kernel
    }

    private fun blur(plane: DoubleArray, width: Int, height: Int, sigma: Double): DoubleArray {
        if (sigma < 0.05) return plane.copyOf()
        val kernel = gaussianKernel(sigma)
        val radius = kernel.size / 2
        val horizontal = DoubleArray(plane.size)
        for (y in 0 until height) {
            for (x in 0 until width) {
                var acc = 0.0
                for (k in kernel.indices) acc += kernel[k] * plane[y * width + (x + k - radius).coerceIn(0, width - 1)]
                horizontal[y * width + x] = acc
            }
        }
        val out = DoubleArray(plane.size)
        for (y in 0 until height) {
            for (x in 0 until width) {
                var acc = 0.0
                for (k in kernel.indices) acc += kernel[k] * horizontal[(y + k - radius).coerceIn(0, height - 1) * width + x]
                out[y * width + x] = acc
            }
        }
        return out
    }

    /** The ground-truth homography from canvas units to ideal image pixels. */
    fun cameraHomography(config: PetalCaptureConfig): PetalHomography {
        val w = config.width.toDouble()
        val h = config.height.toDouble()
        val focal = 0.8 * Math.max(w, h)
        val distance = focal * 1024.0 / (config.fill * Math.min(w, h))
        val rz = Math.toRadians(config.rotationDeg)
        val rx = Math.toRadians(config.tiltXDeg)
        val ry = Math.toRadians(config.tiltYDeg)
        val rotX = arrayOf(doubleArrayOf(1.0, 0.0, 0.0), doubleArrayOf(0.0, Math.cos(rx), -Math.sin(rx)), doubleArrayOf(0.0, Math.sin(rx), Math.cos(rx)))
        val rotY = arrayOf(doubleArrayOf(Math.cos(ry), 0.0, Math.sin(ry)), doubleArrayOf(0.0, 1.0, 0.0), doubleArrayOf(-Math.sin(ry), 0.0, Math.cos(ry)))
        val rotZ = arrayOf(doubleArrayOf(Math.cos(rz), -Math.sin(rz), 0.0), doubleArrayOf(Math.sin(rz), Math.cos(rz), 0.0), doubleArrayOf(0.0, 0.0, 1.0))
        fun mul(a: Array<DoubleArray>, b: Array<DoubleArray>) = Array(3) { i -> DoubleArray(3) { j -> a[i][0] * b[0][j] + a[i][1] * b[1][j] + a[i][2] * b[2][j] } }
        val r = mul(rotZ, mul(rotY, rotX))
        val t = doubleArrayOf(
            -512.0 * r[0][0] - 512.0 * r[0][1],
            -512.0 * r[1][0] - 512.0 * r[1][1],
            distance - 512.0 * r[2][0] - 512.0 * r[2][1],
        )
        val k = arrayOf(
            doubleArrayOf(focal, 0.0, w / 2.0 + config.shiftX * w),
            doubleArrayOf(0.0, focal, h / 2.0 + config.shiftY * h),
            doubleArrayOf(0.0, 0.0, 1.0),
        )
        val m = arrayOf(doubleArrayOf(r[0][0], r[0][1], t[0]), doubleArrayOf(r[1][0], r[1][1], t[1]), doubleArrayOf(r[2][0], r[2][1], t[2]))
        val hm = mul(k, m)
        return PetalHomography(doubleArrayOf(hm[0][0], hm[0][1], hm[0][2], hm[1][0], hm[1][1], hm[1][2], hm[2][0], hm[2][1], hm[2][2]))
    }

    /** Shrinks `fill` until all four canvas corners lie inside the frame. */
    fun fitToFrame(config: PetalCaptureConfig, marginPx: Double): PetalCaptureConfig {
        var fitted = config
        repeat(40) {
            val h = cameraHomography(fitted)
            val inside = listOf(0.0 to 0.0, 1024.0 to 0.0, 1024.0 to 1024.0, 0.0 to 1024.0).all { (x, y) ->
                val p = h.apply(x, y)
                p[0] >= marginPx && p[1] >= marginPx && p[0] <= fitted.width - marginPx && p[1] <= fitted.height - marginPx
            }
            if (inside) return fitted
            fitted = fitted.copy(fill = fitted.fill * 0.97)
        }
        return fitted
    }

    /** Captures [source] (a rendered frame) with the simulated camera. */
    fun capture(source: PetalRgb, config: PetalCaptureConfig): PetalLuma {
        val w = config.width
        val h = config.height
        val focal = 0.8 * Math.max(w, h)
        val backward = checkNotNull(cameraHomography(config).inverse())
        val cxImage = w / 2.0 + config.shiftX * w
        val cyImage = h / 2.0 + config.shiftY * h
        val srcW = source.width
        val srcH = source.height
        val rgb = source.data()
        val linear = DoubleArray(srcW * srcH) { i ->
            val value = (0.299 * (rgb[3 * i].toInt() and 0xFF) + 0.587 * (rgb[3 * i + 1].toInt() and 0xFF) +
                0.114 * (rgb[3 * i + 2].toInt() and 0xFF)) / 255.0
            Math.pow(value, 2.2)
        }
        val scale = srcW / 1024.0
        fun sampleSource(x: Double, y: Double): Double {
            val sx = x * scale
            val sy = y * scale
            if (sx < 0.0 || sy < 0.0 || sx >= srcW || sy >= srcH) return 0.0
            val fx = (sx - 0.5).coerceIn(0.0, (srcW - 1).toDouble())
            val fy = (sy - 0.5).coerceIn(0.0, (srcH - 1).toDouble())
            val x0 = fx.toInt()
            val y0 = fy.toInt()
            val x1 = minOf(x0 + 1, srcW - 1)
            val y1 = minOf(y0 + 1, srcH - 1)
            val tx = fx - x0
            val ty = fy - y0
            return (linear[y0 * srcW + x0] * (1.0 - tx) + linear[y0 * srcW + x1] * tx) * (1.0 - ty) +
                (linear[y1 * srcW + x0] * (1.0 - tx) + linear[y1 * srcW + x1] * tx) * ty
        }
        var plane = DoubleArray(w * h)
        for (y in 0 until h) {
            for (x in 0 until w) {
                var acc = 0.0
                for (sy in 0 until SS) {
                    for (sx in 0 until SS) {
                        var px = x + (sx + 0.5) / SS
                        var py = y + (sy + 0.5) / SS
                        if (config.lensK1 != 0.0) {
                            val dx = px - cxImage
                            val dy = py - cyImage
                            var ux = dx
                            var uy = dy
                            repeat(5) {
                                val factor = 1.0 + config.lensK1 * (ux * ux + uy * uy) / (focal * focal)
                                ux = dx / factor
                                uy = dy / factor
                            }
                            px = cxImage + ux
                            py = cyImage + uy
                        }
                        val canvas = backward.apply(px, py)
                        acc += sampleSource(canvas[0], canvas[1])
                    }
                }
                plane[y * w + x] = acc / (SS * SS)
            }
        }
        val optical = blur(plane, w, h, config.blurSigma)
        plane = if (config.bloom > 0.0) {
            val halo = blur(plane, w, h, config.bloomSigma)
            DoubleArray(plane.size) { (1.0 - config.bloom) * optical[it] + config.bloom * halo[it] }
        } else {
            optical
        }
        // auto exposure: map the 99.5th percentile to 0.85, then apply the multiplier
        val sorted = plane.copyOf().also { it.sort() }
        val p995 = Math.max(sorted[((sorted.size - 1) * 0.995).toInt()], 1e-4)
        val gain = 0.85 / p995 * config.exposure
        val gx = Math.cos(Math.toRadians(config.gradientDeg))
        val gy = Math.sin(Math.toRadians(config.gradientDeg))
        val rng = Rng(config.seed)
        val diagX = w / 2.0
        val diagY = h / 2.0
        val maxR2 = diagX * diagX + diagY * diagY
        val encoded = DoubleArray(w * h)
        for (y in 0 until h) {
            for (x in 0 until w) {
                val dx = x - diagX
                val dy = y - diagY
                var value = plane[y * w + x] * (1.0 - config.vignette * (dx * dx + dy * dy) / maxR2)
                val along = (dx * gx + dy * gy) / Math.sqrt(maxR2)
                value += p995 * (config.ambient + config.gradient * (0.5 + 0.5 * along.coerceIn(-1.0, 1.0)))
                val level = Math.pow((value * gain).coerceIn(0.0, 1.0), 1.0 / 2.2) * 255.0
                val sigma = config.noise * Math.sqrt(0.3 + 0.7 * level / 255.0)
                encoded[y * w + x] = level + sigma * rng.gaussian()
            }
        }
        if (config.sharpen > 0.0) {
            val soft = blur(encoded, w, h, 1.4)
            for (index in encoded.indices) encoded[index] += config.sharpen * (encoded[index] - soft[index])
        }
        val bytes = ByteArray(w * h) { Math.round(encoded[it]).coerceIn(0L, 255L).toByte() }
        return checkNotNull(PetalLuma.fromRaw(w, h, bytes))
    }
}
