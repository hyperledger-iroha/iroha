package org.hyperledger.iroha.sdk.tx

import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.norito.NoritoException
import org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder

/** Canonical hashing helpers for signed transactions. */
object SignedTransactionHasher {

    /** Computes the canonical BLAKE2b-256 hash bytes for the given signed transaction. */
    @JvmStatic
    fun hash(transaction: SignedTransaction): ByteArray {
        val canonicalBytes = canonicalBytes(transaction)
        return IrohaHash.prehash(canonicalBytes)
    }

    /** Computes the canonical BLAKE2b-256 hash as a lowercase hex string. */
    @JvmStatic
    fun hashHex(transaction: SignedTransaction): String = toHex(hash(transaction))

    /** Computes the canonical hash for pre-encoded signed transaction bytes. */
    @JvmStatic
    fun hashCanonicalBytes(canonicalSignedTransaction: ByteArray): ByteArray =
        IrohaHash.prehash(canonicalSignedTransaction)

    /** Computes the canonical hash hex for pre-encoded signed transaction bytes. */
    @JvmStatic
    fun hashCanonicalHex(canonicalSignedTransaction: ByteArray): String =
        toHex(hashCanonicalBytes(canonicalSignedTransaction))

    /**
     * Returns the canonical Norito bytes for the signed transaction.
     *
     * Iroha hashes the `TransactionEntrypoint::External` enum wrapper around the signed
     * transaction, not the signed transaction directly. The encoding is:
     * `u32_LE(0) + u64_LE(payload.length) + payload`.
     */
    @JvmStatic
    fun canonicalBytes(transaction: SignedTransaction): ByteArray {
        try {
            val encoded = SignedTransactionEncoder.encode(transaction)
            val result = ByteArray(12 + encoded.size)
            // u32 LE discriminant = 0 (External variant) -- result[0..3] already zeroed
            val length = encoded.size.toLong()
            for (i in 0 until 8) {
                result[4 + i] = (length ushr (i * 8)).toByte()
            }
            System.arraycopy(encoded, 0, result, 12, encoded.size)
            return result
        } catch (ex: NoritoException) {
            throw IllegalStateException("Failed to encode signed transaction", ex)
        }
    }

    private fun toHex(data: ByteArray): String {
        val builder = StringBuilder(data.size * 2)
        for (b in data) {
            builder.append(String.format("%02x", b))
        }
        return builder.toString()
    }
}
