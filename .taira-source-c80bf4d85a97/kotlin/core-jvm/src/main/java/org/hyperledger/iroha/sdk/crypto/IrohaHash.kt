package org.hyperledger.iroha.sdk.crypto

/** Iroha hashing helpers (Blake2b-256 with prehash marker). */
object IrohaHash {

    /**
     * Hashes the provided bytes using Blake2b-256 and sets the least significant bit to 1.
     *
     * @param message bytes to hash
     * @return hashed bytes with prehash marker applied
     */
    @JvmStatic
    fun prehash(message: ByteArray): ByteArray {
        val digest = Blake2b.digest256(message)
        digest[digest.size - 1] = (digest[digest.size - 1].toInt() or 1).toByte()
        return digest
    }
}
