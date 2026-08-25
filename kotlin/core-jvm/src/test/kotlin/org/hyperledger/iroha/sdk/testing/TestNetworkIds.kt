package org.hyperledger.iroha.sdk.testing

import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Deterministic canonical network identities for Kotlin SDK tests. */
object TestNetworkIds {
    private val canonical = NetworkId.parse(
        "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
    )

    /** Returns the canonical development network identity. */
    fun canonical(): NetworkId = canonical

    /** Returns a deterministic distinct canonical identity for the supplied seed. */
    fun fromSeed(seed: Long): NetworkId {
        val bytes = ByteArray(NetworkId.BYTE_LENGTH)
        var state = seed xor -7046029254386353131L
        for (index in bytes.indices) {
            state = state xor (state ushr 12)
            state = state xor (state shl 25)
            state = state xor (state ushr 27)
            bytes[index] = (state * 2685821657736338717L).toByte()
        }
        bytes[bytes.lastIndex] = (bytes.last().toInt() or 0x01).toByte()
        return NetworkId.fromBytes(bytes)
    }
}
