package org.hyperledger.iroha.sdk.core.model

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.client.JsonParser

/** One canonical JSON value suitable for the signed Norito metadata wire. */
@JvmInline
value class JsonValue private constructor(val canonicalJson: String) {
    companion object {
        /** Encodes a string as one canonical JSON value. */
        fun string(value: String): JsonValue {
            val sb = StringBuilder(value.length + 2)
            sb.append('"')
            for (c in value) {
                when (c) {
                    '"' -> sb.append("\\\"")
                    '\\' -> sb.append("\\\\")
                    '\b' -> sb.append("\\b")
                    '\u000C' -> sb.append("\\f")
                    '\n' -> sb.append("\\n")
                    '\r' -> sb.append("\\r")
                    '\t' -> sb.append("\\t")
                    else -> {
                        if (c < ' ') {
                            sb.append("\\u00")
                            sb.append(HEX_DIGITS[(c.code shr 4) and 0xF])
                            sb.append(HEX_DIGITS[c.code and 0xF])
                        } else {
                            sb.append(c)
                        }
                    }
                }
            }
            sb.append('"')
            return parse(sb.toString())
        }

        /** Encodes an integer as one canonical JSON value. */
        fun number(value: Long): JsonValue = JsonValue(value.toString())

        /** Encodes a boolean as one canonical JSON value. */
        fun bool(value: Boolean): JsonValue = JsonValue(if (value) "true" else "false")

        /** Returns the canonical JSON null value. */
        fun nullValue(): JsonValue = JsonValue("null")

        /** Parses a JSON document and discards every alternate lexical spelling. */
        fun parse(json: String): JsonValue = JsonValue(CanonicalMetadataJson.canonicalize(json))

        /**
         * Accepts a decoded signed-wire value only when it already uses the canonical spelling.
         *
         * Binary decoding must reject rather than silently rewrite signed bytes.
         */
        internal fun fromCanonicalWire(json: String): JsonValue {
            val canonical = CanonicalMetadataJson.canonicalize(json)
            require(canonical == json) {
                "JsonValue wire payload is valid but not in canonical lexical form"
            }
            return JsonValue(canonical)
        }

        private val HEX_DIGITS = charArrayOf(
            '0', '1', '2', '3', '4', '5', '6', '7',
            '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
        )
    }
}

/** Canonicalizer matching the Norito `Json` value model and compact writer. */
private object CanonicalMetadataJson {
    private const val MAX_JSON_BYTES = 1_048_576
    private val MAX_U64 = BigInteger("18446744073709551615")
    private val MIN_I64 = BigInteger.valueOf(Long.MIN_VALUE)

    fun canonicalize(json: String): String {
        require(json.length <= MAX_JSON_BYTES) {
            "JsonValue exceeds the $MAX_JSON_BYTES-byte UTF-8 limit"
        }
        require(json.toByteArray(StandardCharsets.UTF_8).size <= MAX_JSON_BYTES) {
            "JsonValue exceeds the $MAX_JSON_BYTES-byte UTF-8 limit"
        }
        val parsed = try {
            JsonParser.parse(json)
        } catch (error: IllegalStateException) {
            throw IllegalArgumentException("JsonValue must contain exactly one valid JSON value", error)
        }
        val canonical = buildString { writeValue(this, parsed) }
        require(canonical.toByteArray(StandardCharsets.UTF_8).size <= MAX_JSON_BYTES) {
            "canonical JsonValue exceeds the $MAX_JSON_BYTES-byte UTF-8 limit"
        }
        return canonical
    }

    @Suppress("UNCHECKED_CAST")
    private fun writeValue(out: StringBuilder, value: Any?) {
        when (value) {
            null -> out.append("null")
            is Boolean -> out.append(if (value) "true" else "false")
            is String -> writeString(out, value)
            is Long -> out.append(value)
            is BigInteger -> writeBigInteger(out, value)
            is BigDecimal -> out.append(formatFinite(value.toDouble()))
            is Double -> out.append(formatFinite(value))
            is Map<*, *> -> {
                out.append('{')
                val keys = value.keys.map { key ->
                    require(key is String) { "JSON object keys must be strings" }
                    key
                }.sortedWith(Comparator(::compareUnicodeScalars))
                keys.forEachIndexed { index, key ->
                    if (index > 0) out.append(',')
                    writeString(out, key)
                    out.append(':')
                    writeValue(out, (value as Map<String, Any?>)[key])
                }
                out.append('}')
            }
            is List<*> -> {
                out.append('[')
                value.forEachIndexed { index, item ->
                    if (index > 0) out.append(',')
                    writeValue(out, item)
                }
                out.append(']')
            }
            else -> throw IllegalArgumentException(
                "unsupported parsed JSON value: ${value::class.java.name}",
            )
        }
    }

    private fun writeBigInteger(out: StringBuilder, value: BigInteger) {
        if (
            (value.signum() >= 0 && value <= MAX_U64) ||
            (value.signum() < 0 && value >= MIN_I64)
        ) {
            out.append(value)
            return
        }
        out.append(formatFinite(value.toDouble()))
    }

    /** Formats a parsed JSON float with Norito's finite-f64 Ryu presentation rules. */
    private fun formatFinite(value: Double): String {
        require(value.isFinite()) { "JSON floating-point number is outside the finite f64 range" }
        val bits = java.lang.Double.doubleToRawLongBits(value)
        val negative = bits < 0
        val magnitude = kotlin.math.abs(value)
        if (magnitude == 0.0) return if (negative) "-0.0" else "0.0"
        // Java's historical spelling for the least subnormal is not the
        // shortest Ryu spelling selected by Norito.
        if (magnitude == Double.MIN_VALUE) return if (negative) "-5e-324" else "5e-324"

        val decimal = BigDecimal.valueOf(magnitude).stripTrailingZeros()
        val digits = decimal.unscaledValue().abs().toString()
        val exponent = -decimal.scale()
        val decimalPoint = digits.length + exponent
        val body = when {
            exponent >= 0 && decimalPoint <= 16 ->
                digits + "0".repeat(decimalPoint - digits.length) + ".0"
            decimalPoint > 0 && decimalPoint <= 16 ->
                digits.substring(0, decimalPoint) + "." + digits.substring(decimalPoint)
            decimalPoint > -5 && decimalPoint <= 0 ->
                "0." + "0".repeat(-decimalPoint) + digits
            digits.length == 1 -> digits + exponentSuffix(decimalPoint - 1)
            else -> digits.first() + "." + digits.substring(1) + exponentSuffix(decimalPoint - 1)
        }
        return if (negative) "-$body" else body
    }

    private fun exponentSuffix(exponent: Int): String =
        if (exponent < 0) "e$exponent" else "e+$exponent"

    private fun compareUnicodeScalars(left: String, right: String): Int {
        var leftIndex = 0
        var rightIndex = 0
        while (leftIndex < left.length && rightIndex < right.length) {
            val leftScalar = Character.codePointAt(left, leftIndex)
            val rightScalar = Character.codePointAt(right, rightIndex)
            if (leftScalar != rightScalar) return leftScalar.compareTo(rightScalar)
            leftIndex += Character.charCount(leftScalar)
            rightIndex += Character.charCount(rightScalar)
        }
        return (left.length - leftIndex).compareTo(right.length - rightIndex)
    }

    private fun writeString(out: StringBuilder, value: String) {
        out.append('"')
        for (character in value) {
            when (character) {
                '"' -> out.append("\\\"")
                '\\' -> out.append("\\\\")
                '\b' -> out.append("\\b")
                '\u000C' -> out.append("\\f")
                '\n' -> out.append("\\n")
                '\r' -> out.append("\\r")
                '\t' -> out.append("\\t")
                else -> {
                    if (character < ' ') {
                        out.append("\\u00")
                        out.append(HEX_DIGITS[(character.code shr 4) and 0xF])
                        out.append(HEX_DIGITS[character.code and 0xF])
                    } else {
                        out.append(character)
                    }
                }
            }
        }
        out.append('"')
    }

    private val HEX_DIGITS = charArrayOf(
        '0', '1', '2', '3', '4', '5', '6', '7',
        '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    )
}
