package org.hyperledger.iroha.sdk.offline.petal

/** Floating-point helpers with the reference (Rust `f64`) semantics. */
internal object PetalNumerics {
    /** Rust `f64::total_cmp`: IEEE 754 totalOrder over the raw bits. */
    @JvmStatic
    fun totalCompare(a: Double, b: Double): Int {
        var left = java.lang.Double.doubleToRawLongBits(a)
        var right = java.lang.Double.doubleToRawLongBits(b)
        left = left xor ((left shr 63) ushr 1)
        right = right xor ((right shr 63) ushr 1)
        return left.compareTo(right)
    }

    /** Rust `f64::min`: a NaN operand is ignored. */
    @JvmStatic
    fun min(a: Double, b: Double): Double = if (a.isNaN()) b else if (b.isNaN()) a else Math.min(a, b)

    /** Rust `f64::max`: a NaN operand is ignored. */
    @JvmStatic
    fun max(a: Double, b: Double): Double = if (a.isNaN()) b else if (b.isNaN()) a else Math.max(a, b)

    /** Rust `f64::round`: half-way cases round away from zero. */
    @JvmStatic
    fun roundHalfAway(value: Double): Double {
        if (value.isNaN() || value.isInfinite()) return value
        val magnitude = Math.abs(value)
        val floor = Math.floor(magnitude)
        val rounded = if (magnitude - floor >= 0.5) floor + 1.0 else floor
        return if (value < 0.0) -rounded else rounded
    }
}
