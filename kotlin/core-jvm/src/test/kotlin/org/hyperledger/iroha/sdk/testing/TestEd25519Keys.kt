package org.hyperledger.iroha.sdk.testing

import org.bouncycastle.crypto.params.Ed25519PrivateKeyParameters

/** Deterministic valid Ed25519 keys for SDK tests that do not exercise key admission. */
object TestEd25519Keys {
    /** Derives a valid public key from a 32-byte seed filled with [seedByte]. */
    @JvmStatic
    fun publicKey(seedByte: Int): ByteArray {
        val seed = ByteArray(32) { seedByte.toByte() }
        return Ed25519PrivateKeyParameters(seed, 0).generatePublicKey().encoded
    }

    /** Derives a valid public key and returns its canonical lowercase hex encoding. */
    @JvmStatic
    fun publicKeyHex(seedByte: Int): String =
        publicKey(seedByte).joinToString(separator = "") { byte ->
            val value = byte.toInt() and 0xFF
            "0123456789abcdef"[value ushr 4].toString() +
                "0123456789abcdef"[value and 0x0F]
        }
}
