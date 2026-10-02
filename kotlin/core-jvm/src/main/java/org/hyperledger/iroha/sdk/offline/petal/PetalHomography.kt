package org.hyperledger.iroha.sdk.offline.petal

/**
 * A 3×3 projective plane transform stored row-major.
 *
 * Petal decoders map canvas design units to camera pixel-edge coordinates
 * with it. All arithmetic is IEEE double precision in the reference order of
 * operations. Immutable.
 */
class PetalHomography(matrix: DoubleArray) {
    /** Row-major coefficients; never exposed without copying. */
    internal val m: DoubleArray = matrix.copyOf()

    init {
        require(matrix.size == 9) { "a homography has 9 coefficients" }
    }

    /** Coefficient [index] (`0..8`, row-major). */
    operator fun get(index: Int): Double = m[index]

    /** The row-major coefficients (a copy). */
    fun toArray(): DoubleArray = m.copyOf()

    /** Maps the point `(x, y)`, returning `[x', y']`. */
    fun apply(x: Double, y: Double): DoubleArray {
        val w = m[6] * x + m[7] * y + m[8]
        return doubleArrayOf((m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w)
    }

    /** The inverse transform, or `null` when singular. */
    fun inverse(): PetalHomography? {
        val c00 = m[4] * m[8] - m[5] * m[7]
        val c01 = m[5] * m[6] - m[3] * m[8]
        val c02 = m[3] * m[7] - m[4] * m[6]
        val det = m[0] * c00 + m[1] * c01 + m[2] * c02
        if (Math.abs(det) < 1e-18) return null
        val inv = 1.0 / det
        return PetalHomography(
            doubleArrayOf(
                c00 * inv,
                (m[2] * m[7] - m[1] * m[8]) * inv,
                (m[1] * m[5] - m[2] * m[4]) * inv,
                c01 * inv,
                (m[0] * m[8] - m[2] * m[6]) * inv,
                (m[2] * m[3] - m[0] * m[5]) * inv,
                c02 * inv,
                (m[1] * m[6] - m[0] * m[7]) * inv,
                (m[0] * m[4] - m[1] * m[3]) * inv,
            ),
        )
    }

    /** `this * other` (apply [other] first). */
    fun compose(other: PetalHomography): PetalHomography = PetalHomography(compose(m, other.m))

    override fun equals(other: Any?): Boolean = other is PetalHomography && m.contentEquals(other.m)

    override fun hashCode(): Int = m.contentHashCode()

    override fun toString(): String = "PetalHomography(${m.joinToString()})"

    companion object {
        /** The identity transform. */
        @JvmField
        val IDENTITY = PetalHomography(doubleArrayOf(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0))

        private const val SQRT_2 = 1.4142135623730951

        /**
         * Fits the homography taking `src[i]` to `dst[i]` (least squares for more
         * than four pairs, exact for four), using Hartley-normalised DLT and an
         * 8×8 Gaussian elimination with partial pivoting.
         *
         * Points are flattened `x0, y0, x1, y1, …`. Returns `null` for fewer than
         * four pairs, mismatched lengths or degenerate configurations.
         */
        @JvmStatic
        fun fromPoints(src: DoubleArray, dst: DoubleArray): PetalHomography? {
            if (src.size != dst.size || src.size % 2 != 0 || src.size < 8) return null
            val ts = normalisation(src)
            val td = normalisation(dst)
            val ata = Array(8) { DoubleArray(8) }
            val atb = DoubleArray(8)
            val row = DoubleArray(8)
            for (point in 0 until src.size / 2) {
                // apply_affine: (scale * x + tx, scale * y + ty)
                val x = ts[0] * src[2 * point] + ts[2]
                val y = ts[4] * src[2 * point + 1] + ts[5]
                val u = td[0] * dst[2 * point] + td[2]
                val v = td[4] * dst[2 * point + 1] + td[5]
                for (equation in 0 until 2) {
                    val rhs: Double
                    if (equation == 0) {
                        row[0] = x; row[1] = y; row[2] = 1.0; row[3] = 0.0
                        row[4] = 0.0; row[5] = 0.0; row[6] = -u * x; row[7] = -u * y
                        rhs = u
                    } else {
                        row[0] = 0.0; row[1] = 0.0; row[2] = 0.0; row[3] = x
                        row[4] = y; row[5] = 1.0; row[6] = -v * x; row[7] = -v * y
                        rhs = v
                    }
                    for (i in 0 until 8) {
                        val target = ata[i]
                        for (j in 0 until 8) target[j] += row[i] * row[j]
                        atb[i] += row[i] * rhs
                    }
                }
            }
            val h = solve8(ata, atb) ?: return null
            val normalised = doubleArrayOf(h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0)
            // H = Td^-1 * Hn * Ts
            val scaleD = td[0]
            val tdInverse = doubleArrayOf(
                1.0 / scaleD, 0.0, -td[2] / scaleD,
                0.0, 1.0 / scaleD, -td[5] / scaleD,
                0.0, 0.0, 1.0,
            )
            val scaleS = ts[0]
            val tsMatrix = doubleArrayOf(scaleS, 0.0, ts[2], 0.0, scaleS, ts[5], 0.0, 0.0, 1.0)
            val composed = compose(compose(tdInverse, normalised), tsMatrix)
            val norm = composed[8]
            if (Math.abs(norm) < 1e-15) return null
            for (index in 0 until 9) composed[index] = composed[index] / norm
            return PetalHomography(composed)
        }

        private fun compose(a: DoubleArray, b: DoubleArray): DoubleArray {
            val out = DoubleArray(9)
            for (r in 0 until 3) {
                for (c in 0 until 3) {
                    // Iterator sums start from -0.0, which is the identity for every addend.
                    out[r * 3 + c] = a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c]
                }
            }
            return out
        }

        /** Translation to the centroid and isotropic scale to mean distance √2. */
        private fun normalisation(points: DoubleArray): DoubleArray {
            val count = points.size / 2
            val n = count.toDouble()
            var cx = 0.0
            var cy = 0.0
            for (point in 0 until count) {
                cx += points[2 * point] / n
                cy += points[2 * point + 1] / n
            }
            var total = -0.0
            for (point in 0 until count) {
                val dx = points[2 * point] - cx
                val dy = points[2 * point + 1] - cy
                total += Math.sqrt(dx * dx + dy * dy)
            }
            val mean = total / n
            val scale = if (mean > 1e-12) SQRT_2 / mean else 1.0
            return doubleArrayOf(scale, 0.0, -scale * cx, 0.0, scale, -scale * cy, 0.0, 0.0, 1.0)
        }

        /** Gaussian elimination with partial pivoting for an 8×8 system. */
        private fun solve8(a: Array<DoubleArray>, b: DoubleArray): DoubleArray? {
            for (col in 0 until 8) {
                // `max_by` keeps the last of equally large candidates.
                var pivot = col
                for (candidate in col + 1 until 8) {
                    if (PetalNumerics.totalCompare(Math.abs(a[candidate][col]), Math.abs(a[pivot][col])) >= 0) {
                        pivot = candidate
                    }
                }
                if (Math.abs(a[pivot][col]) < 1e-14) return null
                val swapRow = a[col]
                a[col] = a[pivot]
                a[pivot] = swapRow
                val swapValue = b[col]
                b[col] = b[pivot]
                b[pivot] = swapValue
                val pivotRow = a[col]
                for (row in col + 1 until 8) {
                    val target = a[row]
                    val factor = target[col] / pivotRow[col]
                    for (k in col until 8) target[k] -= factor * pivotRow[k]
                    b[row] -= factor * b[col]
                }
            }
            val x = DoubleArray(8)
            for (row in 7 downTo 0) {
                var tail = -0.0
                for (k in row + 1 until 8) tail += a[row][k] * x[k]
                x[row] = (b[row] - tail) / a[row][row]
            }
            return x
        }
    }
}
