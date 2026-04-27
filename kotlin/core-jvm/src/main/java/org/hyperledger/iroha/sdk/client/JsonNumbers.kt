package org.hyperledger.iroha.sdk.client

import java.math.BigDecimal
import java.math.BigInteger

/** Checked integer coercions for values emitted by [JsonParser]. */
internal object JsonNumbers {

    private val LONG_MIN = BigInteger.valueOf(Long.MIN_VALUE)
    private val LONG_MAX = BigInteger.valueOf(Long.MAX_VALUE)
    private const val LONG_MIN_DOUBLE = -9223372036854775808.0
    private const val LONG_MAX_EXCLUSIVE_DOUBLE = 9223372036854775808.0

    fun asLong(value: Any?, path: String): Long =
        asLong(value, path, allowFloatingPoint = false)

    fun asLongAllowingIntegralFloat(value: Any?, path: String): Long =
        asLong(value, path, allowFloatingPoint = true)

    fun asInt(value: Any?, path: String): Int =
        checkedInt(asLong(value, path), path)

    fun asIntAllowingIntegralFloat(value: Any?, path: String): Int =
        checkedInt(asLongAllowingIntegralFloat(value, path), path)

    private fun asLong(value: Any?, path: String, allowFloatingPoint: Boolean): Long {
        check(value is Number) { "$path must be a number" }
        return when (value) {
            is BigInteger -> checkedLong(value, path)
            is BigDecimal -> checkedDecimalLong(value, path)
            is Double -> checkedFloatingLong(value, path, allowFloatingPoint)
            is Float -> checkedFloatingLong(value.toDouble(), path, allowFloatingPoint)
            else -> value.toLong()
        }
    }

    private fun checkedLong(value: BigInteger, path: String): Long {
        check(value >= LONG_MIN && value <= LONG_MAX) {
            "$path must fit in signed 64-bit range"
        }
        return value.toLong()
    }

    private fun checkedDecimalLong(value: BigDecimal, path: String): Long {
        val integer = try {
            value.toBigIntegerExact()
        } catch (ex: ArithmeticException) {
            throw IllegalStateException("$path must be an integer", ex)
        }
        return checkedLong(integer, path)
    }

    private fun checkedFloatingLong(value: Double, path: String, allowFloatingPoint: Boolean): Long {
        check(allowFloatingPoint) { "$path must be an integer" }
        check(value.isFinite() && value % 1.0 == 0.0) { "$path must be an integer" }
        check(value >= LONG_MIN_DOUBLE && value < LONG_MAX_EXCLUSIVE_DOUBLE) {
            "$path must fit in signed 64-bit range"
        }
        return value.toLong()
    }

    private fun checkedInt(value: Long, path: String): Int {
        check(value in Int.MIN_VALUE.toLong()..Int.MAX_VALUE.toLong()) {
            "$path must fit in signed 32-bit range"
        }
        return value.toInt()
    }
}
