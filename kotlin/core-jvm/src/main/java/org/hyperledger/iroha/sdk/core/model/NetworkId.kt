package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.util.HashLiteral

/**
 * Exact immutable identity of one Iroha network.
 *
 * A network identity is the canonical 64-character lowercase hexadecimal form of the 32-byte
 * genesis-header hash.
 * Ordinary transactions always carry this value through `TransactionDomain::Network`; the
 * genesis-only transaction domain is intentionally not representable by this type.
 */
class NetworkId private constructor(value: ByteArray) {
    private val value = value.copyOf()

    /** Exact 64-character lowercase hexadecimal literal. */
    val literal: String = encodeLowerHex(this.value)

    /** Canonical checksummed representation used only by Norito JSON. */
    val noritoJsonLiteral: String = HashLiteral.canonicalize(this.value)

    /** Returns a defensive copy of the exact 32-byte identity. */
    fun bytes(): ByteArray = value.copyOf()

    override fun toString(): String = literal

    override fun equals(other: Any?): Boolean =
        this === other || other is NetworkId && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Exact byte width of the genesis-header hash. */
        const val BYTE_LENGTH: Int = 32

        /** Parses one exact 64-character lowercase hexadecimal network identity. */
        @JvmStatic
        fun parse(literal: String): NetworkId {
            require(literal.length == BYTE_LENGTH * 2) {
                INVALID_LITERAL_MESSAGE
            }
            val bytes = ByteArray(BYTE_LENGTH)
            for (index in bytes.indices) {
                val high = decodeLowerHex(literal[index * 2])
                val low = decodeLowerHex(literal[index * 2 + 1])
                require(high >= 0 && low >= 0) { INVALID_LITERAL_MESSAGE }
                bytes[index] = ((high shl 4) or low).toByte()
            }
            require((bytes[BYTE_LENGTH - 1].toInt() and 1) == 1) { INVALID_LITERAL_MESSAGE }
            return NetworkId(bytes)
        }

        /** Parses the distinct canonical checksummed representation emitted by Norito JSON. */
        @JvmStatic
        fun parseNoritoJsonLiteral(literal: String): NetworkId {
            require(NORITO_JSON_LITERAL.matches(literal)) {
                INVALID_NORITO_JSON_LITERAL_MESSAGE
            }
            val bytes = try {
                HashLiteral.decode(literal)
            } catch (error: IllegalArgumentException) {
                throw IllegalArgumentException(INVALID_NORITO_JSON_LITERAL_MESSAGE, error)
            }
            require(HashLiteral.canonicalize(bytes) == literal) {
                INVALID_NORITO_JSON_LITERAL_MESSAGE
            }
            return fromBytes(bytes)
        }

        /** Creates a network identity from its exact canonical 32 raw bytes. */
        @JvmStatic
        fun fromBytes(bytes: ByteArray): NetworkId {
            require(bytes.size == BYTE_LENGTH) {
                "NetworkId raw value must contain exactly $BYTE_LENGTH bytes"
            }
            require((bytes[BYTE_LENGTH - 1].toInt() and 1) == 1) {
                "NetworkId genesis hash marker bit must be set"
            }
            return NetworkId(bytes)
        }

        private const val LOWER_HEX = "0123456789abcdef"
        private const val INVALID_LITERAL_MESSAGE =
            "NetworkId must be exactly 64 lowercase hexadecimal characters with its marker bit set"
        private const val INVALID_NORITO_JSON_LITERAL_MESSAGE =
            "Norito JSON NetworkId must be one canonical checksummed uppercase hash literal"
        private val NORITO_JSON_LITERAL = Regex("^hash:[0-9A-F]{64}#[0-9A-F]{4}$")

        private fun encodeLowerHex(bytes: ByteArray): String =
            buildString(bytes.size * 2) {
                bytes.forEach { item ->
                    val unsigned = item.toInt() and 0xff
                    append(LOWER_HEX[unsigned ushr 4])
                    append(LOWER_HEX[unsigned and 0x0f])
                }
            }

        private fun decodeLowerHex(value: Char): Int =
            when (value) {
                in '0'..'9' -> value - '0'
                in 'a'..'f' -> value - 'a' + 10
                else -> -1
            }
    }
}
