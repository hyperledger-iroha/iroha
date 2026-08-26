package org.hyperledger.iroha.sdk.crypto

import org.bouncycastle.crypto.digests.Blake2bDigest

/** BLAKE2b helpers backed by the SDK's directly linked Bouncy Castle implementation. */
object Blake2b {
    /** Returns the 256-bit BLAKE2b digest of [message]. */
    @JvmStatic
    fun digest256(message: ByteArray): ByteArray = digest(message, 32)

    /** Returns the 512-bit BLAKE2b digest of [message]. */
    @JvmStatic
    fun digest512(message: ByteArray): ByteArray = digest(message, 64)

    /** Returns the canonical 256-bit BLAKE2b digest of [message]. */
    @JvmStatic
    fun digest(message: ByteArray): ByteArray = digest256(message)

    /** Returns a BLAKE2b digest of [message] with exactly [outLen] bytes. */
    @JvmStatic
    fun digest(message: ByteArray, outLen: Int): ByteArray {
        require(outLen in 1..64) { "BLAKE2b output length must be between 1 and 64 bytes" }

        val digest = Blake2bDigest(outLen * Byte.SIZE_BITS)
        digest.update(message, 0, message.size)
        return ByteArray(outLen).also { digest.doFinal(it, 0) }
    }
}
