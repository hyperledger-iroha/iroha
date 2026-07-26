package org.hyperledger.iroha.sdk.alias

import org.hyperledger.iroha.sdk.core.util.HashLiteral

/** Hash text accepted by alias planner JSON and plan verification. */
internal object AliasHashText {
    fun decode(value: String?): ByteArray? {
        if (value == null) return null
        if (value.startsWith("hash:", ignoreCase = true)) {
            return try {
                HashLiteral.decode(value)
            } catch (_: IllegalArgumentException) {
                null
            }
        }
        val raw = when {
            value.startsWith("0x") -> value.substring(2)
            value.startsWith("blake2b:") -> value.substring(8)
            else -> value
        }
        if (raw.length != 64 || raw.any { it !in '0'..'9' && it !in 'a'..'f' && it !in 'A'..'F' }) {
            return null
        }
        return ByteArray(32) { index -> raw.substring(index * 2, index * 2 + 2).toInt(16).toByte() }
    }
}
