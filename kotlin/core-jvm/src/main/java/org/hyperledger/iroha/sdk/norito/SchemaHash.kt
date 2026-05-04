// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.security.MessageDigest
import java.util.Locale

private val TYPE_NAME_SCHEMA_HASH_DOMAIN = "norito:v1:type-name\u0000".toByteArray(Charsets.UTF_8)
private val STRUCTURAL_SCHEMA_HASH_DOMAIN = "norito:v1:structural-schema\u0000".toByteArray(Charsets.UTF_8)

/** Computes domain-separated SHA-256 schema hashes matching the Rust implementation. */
object SchemaHash {

    @JvmStatic
    fun hash16(canonicalPath: String): ByteArray {
        return hash16(TYPE_NAME_SCHEMA_HASH_DOMAIN, canonicalPath.toByteArray(Charsets.UTF_8))
    }

    @JvmStatic
    fun hash16FromStructural(schema: Any): ByteArray {
        val canonical = encodeCanonicalJson(schema)
        return hash16(STRUCTURAL_SCHEMA_HASH_DOMAIN, canonical.toByteArray(Charsets.UTF_8))
    }

    private fun hash16(domain: ByteArray, input: ByteArray): ByteArray {
        val digest = MessageDigest.getInstance("SHA-256")
        digest.update(domain)
        digest.update(input)
        return digest.digest().copyOfRange(0, 16)
    }

    private fun encodeCanonicalJson(value: Any?): String {
        val out = StringBuilder()
        encodeCanonicalJson(value, out)
        return out.toString()
    }

    private fun encodeCanonicalJson(value: Any?, out: StringBuilder) {
        when {
            value == null -> out.append("null")
            value is Boolean -> out.append(if (value) "true" else "false")
            value is String -> encodeJsonString(value, out)
            value is Number && value !is Float && value !is Double -> out.append(value.toString())
            value is Double -> out.append(encodeFloat(value))
            value is Float -> out.append(encodeFloat(value.toDouble()))
            value is Map<*, *> -> {
                out.append('{')
                val sorted = java.util.TreeMap<String, Any?>()
                for ((k, v) in value) {
                    require(k is String) { "Structural schema keys must be strings" }
                    sorted[k] = v
                }
                var first = true
                for ((k, v) in sorted) {
                    if (!first) out.append(',')
                    encodeJsonString(k, out)
                    out.append(':')
                    encodeCanonicalJson(v, out)
                    first = false
                }
                out.append('}')
            }
            value is Iterable<*> -> {
                out.append('[')
                var first = true
                for (item in value) {
                    if (!first) out.append(',')
                    encodeCanonicalJson(item, out)
                    first = false
                }
                out.append(']')
            }
            value is BooleanArray -> encodeCanonicalJson(value.toList(), out)
            value is ByteArray -> encodeCanonicalJson(value.toList(), out)
            value is ShortArray -> encodeCanonicalJson(value.toList(), out)
            value is IntArray -> encodeCanonicalJson(value.toList(), out)
            value is LongArray -> encodeCanonicalJson(value.toList(), out)
            value is FloatArray -> encodeCanonicalJson(value.toList(), out)
            value is DoubleArray -> encodeCanonicalJson(value.toList(), out)
            value is CharArray -> encodeCanonicalJson(value.toList(), out)
            value is Array<*> -> encodeCanonicalJson(value.toList(), out)
            else -> throw IllegalArgumentException("Unsupported structural schema element: ${value.javaClass}")
        }
    }

    private fun encodeFloat(value: Double): String {
        if (value.isNaN()) return "NaN"
        if (value.isInfinite()) return if (value > 0) "inf" else "-inf"
        val abs = Math.abs(value)
        if (Math.rint(value) == value && abs <= 9_007_199_254_740_992.0) {
            return String.format(Locale.ROOT, "%.1f", value)
        }
        return String.format(Locale.ROOT, "%.17g", value)
    }

    private fun encodeJsonString(value: String, out: StringBuilder) {
        out.append('"')
        var i = 0
        while (i < value.length) {
            val codePoint = value.codePointAt(i)
            i += Character.charCount(codePoint)
            when (codePoint) {
                '"'.code -> out.append("\\\"")
                '\\'.code -> out.append("\\\\")
                '\n'.code -> out.append("\\n")
                '\r'.code -> out.append("\\r")
                '\t'.code -> out.append("\\t")
                '\b'.code -> out.append("\\b")
                0x0C -> out.append("\\f") // \f form feed
                0x2028 -> out.append("\\u2028")
                0x2029 -> out.append("\\u2029")
                else -> {
                    if (codePoint < 0x20) {
                        out.append(String.format(Locale.ROOT, "\\u%04X", codePoint))
                    } else if (codePoint >= 0x10000) {
                        val tmp = codePoint - 0x10000
                        val hi = 0xD800 + (tmp shr 10)
                        val lo = 0xDC00 + (tmp and 0x3FF)
                        out.append(String.format(Locale.ROOT, "\\u%04X\\u%04X", hi, lo))
                    } else {
                        out.appendCodePoint(codePoint)
                    }
                }
            }
        }
        out.append('"')
    }
}
