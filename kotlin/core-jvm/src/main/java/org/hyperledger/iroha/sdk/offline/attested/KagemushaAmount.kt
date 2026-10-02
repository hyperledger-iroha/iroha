// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline.attested

import java.math.BigDecimal
import java.math.BigInteger

/**
 * Non-negative KAGEMUSHA attested-app amount in minor units at the asset scale.
 *
 * The protocol range is `0..=10^15` minor units. Zero is a valid balance; every transfer,
 * load and redemption additionally requires a positive amount. Parsing never rounds.
 */
class KagemushaAmount private constructor(
    /** Amount in minor units. */
    @JvmField val minor: Long,
) : Comparable<KagemushaAmount> {

    /** True when this amount is zero. */
    val isZero: Boolean get() = minor == 0L

    /** Exact sum; throws when the result leaves the protocol range. */
    operator fun plus(other: KagemushaAmount): KagemushaAmount = ofMinor(Math.addExact(minor, other.minor))

    /** Exact difference; throws when the result would be negative. */
    operator fun minus(other: KagemushaAmount): KagemushaAmount {
        require(other.minor <= minor) { "KAGEMUSHA amount subtraction would be negative" }
        return ofMinor(minor - other.minor)
    }

    /** Render with exactly [scale] fractional digits, for example `5.00` for 500 at scale 2. */
    fun format(scale: Int): String {
        requireScale(scale)
        return BigDecimal(BigInteger.valueOf(minor), scale).toPlainString()
    }

    override fun compareTo(other: KagemushaAmount): Int = minor.compareTo(other.minor)

    override fun equals(other: Any?): Boolean = other is KagemushaAmount && other.minor == minor

    override fun hashCode(): Int = minor.hashCode()

    override fun toString(): String = "KagemushaAmount($minor)"

    companion object {
        /** Largest amount admitted by the protocol: `10^15` minor units. */
        const val MAXIMUM_MINOR: Long = 1_000_000_000_000_000L

        /** Largest asset scale accepted by [parse] and [format]. */
        const val MAXIMUM_SCALE: Int = 18

        @JvmField
        val ZERO: KagemushaAmount = KagemushaAmount(0L)

        private val DECIMAL = Regex("(0|[1-9][0-9]*)(\\.[0-9]+)?")

        /** Construct from minor units, rejecting values outside `0..=10^15`. */
        @JvmStatic
        fun ofMinor(minor: Long): KagemushaAmount {
            require(minor in 0L..MAXIMUM_MINOR) { "KAGEMUSHA amount is outside 0..=10^15 minor units" }
            return if (minor == 0L) ZERO else KagemushaAmount(minor)
        }

        /**
         * Parse an exact unsigned decimal such as `50`, `50.0` or `50.00` at [scale].
         *
         * Signs, exponents, whitespace, leading zeros, a bare decimal point and more fractional
         * digits than [scale] are rejected instead of being rounded.
         */
        @JvmStatic
        fun parse(decimal: String, scale: Int): KagemushaAmount {
            requireScale(scale)
            require(decimal.length <= 64 && DECIMAL.matches(decimal)) {
                "KAGEMUSHA amount must be an unsigned decimal without exponent or sign"
            }
            val fraction = decimal.substringAfter('.', "")
            require(fraction.length <= scale) {
                "KAGEMUSHA amount has more than $scale fractional digits"
            }
            val minor = BigDecimal(decimal).movePointRight(scale).toBigIntegerExact()
            require(minor.signum() >= 0 && minor <= BigInteger.valueOf(MAXIMUM_MINOR)) {
                "KAGEMUSHA amount is outside 0..=10^15 minor units"
            }
            return ofMinor(minor.toLong())
        }

        private fun requireScale(scale: Int) {
            require(scale in 0..MAXIMUM_SCALE) { "KAGEMUSHA asset scale must be within 0..=$MAXIMUM_SCALE" }
        }
    }
}
