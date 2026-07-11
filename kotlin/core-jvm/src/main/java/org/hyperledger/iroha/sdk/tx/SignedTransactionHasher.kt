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

    /**
     * Computes the canonical hash for exact canonical bare `SignedTransaction` bytes.
     *
     * The input must not include the version byte or an entrypoint wrapper. It is decoded and
     * re-encoded before hashing so truncated, non-canonical, versioned, and double-wrapped inputs
     * fail closed.
     */
    @JvmStatic
    fun hashCanonicalBytes(canonicalBareSignedTransaction: ByteArray): ByteArray =
        IrohaHash.prehash(canonicalBytesFromBare(canonicalBareSignedTransaction))

    /** Computes the canonical hash hex for exact canonical bare signed transaction bytes. */
    @JvmStatic
    fun hashCanonicalHex(canonicalBareSignedTransaction: ByteArray): String =
        toHex(hashCanonicalBytes(canonicalBareSignedTransaction))

    /**
     * Validates and wraps exact canonical bare `SignedTransaction` bytes as
     * `TransactionEntrypoint::External`.
     */
    @JvmStatic
    fun canonicalBytesFromBare(canonicalBareSignedTransaction: ByteArray): ByteArray {
        val snapshot = canonicalBareSignedTransaction.copyOf()
        try {
            val decoded = SignedTransactionEncoder.decode(snapshot)
            val reencoded = SignedTransactionEncoder.encode(decoded)
            require(snapshot.contentEquals(reencoded)) {
                "signed transaction bytes are not the exact canonical bare encoding"
            }
            return wrapExternalEntrypoint(snapshot)
        } catch (ex: NoritoException) {
            throw IllegalArgumentException(
                "signed transaction bytes are not a valid canonical bare encoding",
                ex,
            )
        }
    }

    /**
     * Returns the canonical Norito bytes for the signed transaction.
     *
     * Iroha hashes the `TransactionEntrypoint::External` enum wrapper around the signed
     * transaction, not the signed transaction directly. The encoding is:
     * `u32_LE(0) + COMPACT_LEN(payload.length) + payload`.
     */
    @JvmStatic
    fun canonicalBytes(transaction: SignedTransaction): ByteArray {
        try {
            val encoded = SignedTransactionEncoder.encode(transaction)
            return canonicalBytesFromBare(encoded)
        } catch (ex: NoritoException) {
            throw IllegalStateException("Failed to encode signed transaction", ex)
        } catch (ex: IllegalArgumentException) {
            throw IllegalStateException("Failed to encode signed transaction", ex)
        }
    }

    private fun wrapExternalEntrypoint(canonicalBareSignedTransaction: ByteArray): ByteArray {
        val lengthPrefix = encodeCompactLength(canonicalBareSignedTransaction.size.toLong())
        val result = ByteArray(4 + lengthPrefix.size + canonicalBareSignedTransaction.size)
        // u32 LE discriminant = 0 (External variant) -- result[0..3] already zeroed
        System.arraycopy(lengthPrefix, 0, result, 4, lengthPrefix.size)
        System.arraycopy(
            canonicalBareSignedTransaction,
            0,
            result,
            4 + lengthPrefix.size,
            canonicalBareSignedTransaction.size,
        )
        return result
    }

    /** Encodes a canonical Norito `COMPACT_LEN` value using minimal unsigned LEB128. */
    internal fun encodeCompactLength(value: Long): ByteArray {
        require(value >= 0) { "compact length must be non-negative" }
        var remaining = value
        val output = ByteArray(10)
        var count = 0
        do {
            var byte = (remaining and 0x7f).toInt()
            remaining = remaining ushr 7
            if (remaining != 0L) {
                byte = byte or 0x80
            }
            output[count++] = byte.toByte()
        } while (remaining != 0L)
        return output.copyOf(count)
    }

    private fun toHex(data: ByteArray): String {
        val builder = StringBuilder(data.size * 2)
        for (b in data) {
            builder.append(String.format("%02x", b))
        }
        return builder.toString()
    }
}
