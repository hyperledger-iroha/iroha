package org.hyperledger.iroha.sdk.offline

import java.math.BigInteger

/** Exact positive Kagemusha amount in authoritative asset-scale atomic units. */
internal class KagemushaScaledAmount private constructor(
    val atomicUnits: String,
    val scale: Int,
) {
    companion object {
        const val MAXIMUM_SCALE: Int = 28
        const val MAXIMUM_ATOMIC_UNITS: String =
            "340282366920938463463374607431768211455"
        private val maximumAtomicUnits = BigInteger(MAXIMUM_ATOMIC_UNITS)

        @JvmStatic
        fun fromAtomicUnits(atomicUnits: String, scale: Int): KagemushaScaledAmount {
            requireScale(scale)
            require(isCanonicalPositiveInteger(atomicUnits)) {
                "atomicUnits must be a canonical positive integer"
            }
            require(BigInteger(atomicUnits) <= maximumAtomicUnits) {
                "atomicUnits must fit in u128"
            }
            return KagemushaScaledAmount(atomicUnits, scale)
        }

        /** Converts exactly and rejects excess precision; this function never rounds. */
        @JvmStatic
        fun fromDecimal(decimal: String, scale: Int): KagemushaScaledAmount {
            requireScale(scale)
            require(decimal.isNotEmpty() && decimal.count { it == '.' } <= 1) {
                "decimal must be canonical and positive"
            }
            val separator = decimal.indexOf('.')
            val whole = if (separator < 0) decimal else decimal.substring(0, separator)
            val fractional = if (separator < 0) "" else decimal.substring(separator + 1)
            require(
                whole.isNotEmpty() &&
                    whole.all(::isAsciiDigit) &&
                    (whole == "0" || whole[0] != '0') &&
                    (separator < 0 || fractional.isNotEmpty()) &&
                    fractional.all(::isAsciiDigit),
            ) { "decimal must be canonical and positive" }
            require(fractional.length <= scale) {
                "decimal has more fractional digits than the asset scale"
            }
            val combined = (whole + fractional + "0".repeat(scale - fractional.length))
                .trimStart('0')
                .ifEmpty { "0" }
            return fromAtomicUnits(combined, scale)
        }

        @JvmStatic
        fun sum(amounts: List<KagemushaScaledAmount>): KagemushaScaledAmount {
            require(amounts.isNotEmpty()) { "amounts must not be empty" }
            val scale = amounts.first().scale
            var total = BigInteger.ZERO
            for (amount in amounts) {
                require(amount.scale == scale) { "amount scales must match" }
                total = total.add(BigInteger(amount.atomicUnits))
                require(total <= maximumAtomicUnits) { "amount sum must fit in u128" }
            }
            return fromAtomicUnits(total.toString(), scale)
        }

        private fun requireScale(scale: Int) {
            require(scale in 0..MAXIMUM_SCALE) {
                "scale must be between 0 and $MAXIMUM_SCALE"
            }
        }

        private fun isCanonicalPositiveInteger(value: String): Boolean =
            value.isNotEmpty() &&
                value.all(::isAsciiDigit) &&
                value != "0" &&
                (value.length == 1 || value[0] != '0')

        private fun isAsciiDigit(value: Char): Boolean = value in '0'..'9'
    }

    /**
     * Exact fixed-scale decimal at the authoritative asset scale.
     *
     * This projection is proof-side evidence; use [displayDecimal] for the
     * canonical public Quantity spelling.
     */
    val fixedScaleDecimal: String
        get() {
            if (scale == 0) return atomicUnits
            val padded = atomicUnits.padStart(scale + 1, '0')
            val split = padded.length - scale
            return padded.substring(0, split) + "." + padded.substring(split)
        }

    /** Canonical public Quantity spelling without insignificant zeroes. */
    val displayDecimal: String
        get() {
            if (scale == 0) return atomicUnits
            return fixedScaleDecimal.trimEnd('0').trimEnd('.')
        }

    fun adding(other: KagemushaScaledAmount): KagemushaScaledAmount = sum(listOf(this, other))

    override fun equals(other: Any?): Boolean =
        other is KagemushaScaledAmount &&
            atomicUnits == other.atomicUnits &&
            scale == other.scale

    override fun hashCode(): Int = 31 * atomicUnits.hashCode() + scale
}
