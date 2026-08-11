package org.hyperledger.iroha.sdk.core.model

import org.hyperledger.iroha.sdk.core.util.HashLiteral

/**
 * Exact immutable identity of one Iroha network.
 *
 * A network identity is the canonical checksummed literal of the 32-byte genesis-header hash.
 * Ordinary transactions always carry this value through `TransactionDomain::Network`; the
 * genesis-only transaction domain is intentionally not representable by this type.
 */
class NetworkId private constructor(value: ByteArray) {
    private val value = value.copyOf()

    /** Exact canonical checksummed literal. */
    val literal: String = HashLiteral.canonicalize(this.value)

    /** Returns a defensive copy of the exact 32-byte identity. */
    fun bytes(): ByteArray = value.copyOf()

    override fun toString(): String = literal

    override fun equals(other: Any?): Boolean =
        this === other || other is NetworkId && value.contentEquals(other.value)

    override fun hashCode(): Int = value.contentHashCode()

    companion object {
        /** Exact byte width of the genesis-header hash. */
        const val BYTE_LENGTH: Int = 32

        /** Parses one exact canonical `hash:...#....` network identity. */
        @JvmStatic
        fun parse(literal: String): NetworkId {
            val bytes = try {
                HashLiteral.decode(literal)
            } catch (ex: IllegalArgumentException) {
                throw IllegalArgumentException(
                    "NetworkId must be an exact canonical checksummed 32-byte hash literal",
                    ex,
                )
            }
            require(
                (bytes[BYTE_LENGTH - 1].toInt() and 1) == 1 &&
                    HashLiteral.canonicalize(bytes) == literal,
            ) {
                "NetworkId must be an exact canonical checksummed 32-byte hash literal"
            }
            return NetworkId(bytes)
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
    }
}
