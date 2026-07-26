package org.hyperledger.iroha.sdk.crypto

import java.security.MessageDigest
import java.security.NoSuchAlgorithmException

/** Minimal BLAKE2b digest implementation (supports 256- and 512-bit output). */
object Blake2b {

    private const val BLOCK_BYTES = 128
    private const val ROUNDS = 12

    private val IV = longArrayOf(
        0x6a09e667f3bcc908L,
        -0x4498517a7b3558c5L,
        0x3c6ef372fe94f82bL,
        -0x5ab00ac5a0e2c90fL,
        0x510e527fade682d1L,
        -0x64fa9773d4c193e1L,
        0x1f83d9abfb41bd6bL,
        0x5be0cd19137e2179L,
    )

    private val SIGMA = arrayOf(
        byteArrayOf(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15),
        byteArrayOf(14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3),
        byteArrayOf(11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4),
        byteArrayOf(7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8),
        byteArrayOf(9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13),
        byteArrayOf(2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9),
        byteArrayOf(12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11),
        byteArrayOf(13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10),
        byteArrayOf(6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5),
        byteArrayOf(10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0),
        byteArrayOf(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15),
        byteArrayOf(14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3),
    )

    @JvmStatic
    fun digest256(message: ByteArray): ByteArray {
        val providerDigest = tryMessageDigest("BLAKE2B-256", message)
        return providerDigest ?: digest(message, 32)
    }

    @JvmStatic
    fun digest512(message: ByteArray): ByteArray {
        val providerDigest = tryMessageDigest("BLAKE2B-512", message)
        return providerDigest ?: digest(message, 64)
    }

    @JvmStatic
    fun digest(message: ByteArray): ByteArray = digest256(message)

    @JvmStatic
    fun digest(message: ByteArray, outLen: Int): ByteArray {
        require(outLen in 1..64) { "BLAKE2b output length must be between 1 and 64 bytes" }

        val h = IV.copyOf()
        h[0] = h[0] xor (0x01010000L or outLen.toLong())

        var t0 = 0L
        var t1 = 0L
        val total = message.size

        if (total > 0) {
            var offset = 0
            val fullBlocks = total / BLOCK_BYTES
            val remainder = total % BLOCK_BYTES

            for (i in 0 until fullBlocks) {
                val isLastFull = (i == fullBlocks - 1) && remainder == 0
                t0 += BLOCK_BYTES
                if (t0 < BLOCK_BYTES) {
                    t1++
                }
                compress(h, message, offset, t0, t1, isLastFull)
                offset += BLOCK_BYTES
            }

            if (remainder > 0) {
                val block = ByteArray(BLOCK_BYTES)
                message.copyInto(block, destinationOffset = 0, startIndex = total - remainder, endIndex = total)
                t0 += remainder
                if (t0 < remainder) {
                    t1++
                }
                compress(h, block, 0, t0, t1, last = true)
            }
        } else {
            val block = ByteArray(BLOCK_BYTES)
            compress(h, block, 0, 0, 0, last = true)
        }

        return toOutput(h, outLen)
    }

    private fun tryMessageDigest(algorithm: String, message: ByteArray): ByteArray? {
        return try {
            val digest = MessageDigest.getInstance(algorithm)
            digest.digest(message)
        } catch (_: NoSuchAlgorithmException) {
            null
        }
    }

    private fun toOutput(h: LongArray, outLen: Int): ByteArray {
        val out = ByteArray(outLen)
        var idx = 0
        for (i in h.indices) {
            if (idx >= outLen) break
            var value = h[i]
            for (j in 0 until 8) {
                if (idx >= outLen) break
                out[idx++] = (value and 0xFF).toByte()
                value = value ushr 8
            }
        }
        return out
    }

    private fun compress(
        h: LongArray,
        block: ByteArray,
        blockOffset: Int,
        t0: Long,
        t1: Long,
        last: Boolean,
    ) {
        val m = LongArray(16) { readLongLE(block, blockOffset + it * 8) }

        val v = LongArray(16)
        h.copyInto(v, destinationOffset = 0, startIndex = 0, endIndex = 8)
        IV.copyInto(v, destinationOffset = 8, startIndex = 0, endIndex = 8)
        v[12] = v[12] xor t0
        v[13] = v[13] xor t1
        if (last) {
            v[14] = v[14].inv()
        }

        for (round in 0 until ROUNDS) {
            val s = SIGMA[round]
            g(v, 0, 4, 8, 12, m[s[0].toInt()], m[s[1].toInt()])
            g(v, 1, 5, 9, 13, m[s[2].toInt()], m[s[3].toInt()])
            g(v, 2, 6, 10, 14, m[s[4].toInt()], m[s[5].toInt()])
            g(v, 3, 7, 11, 15, m[s[6].toInt()], m[s[7].toInt()])
            g(v, 0, 5, 10, 15, m[s[8].toInt()], m[s[9].toInt()])
            g(v, 1, 6, 11, 12, m[s[10].toInt()], m[s[11].toInt()])
            g(v, 2, 7, 8, 13, m[s[12].toInt()], m[s[13].toInt()])
            g(v, 3, 4, 9, 14, m[s[14].toInt()], m[s[15].toInt()])
        }

        for (i in 0 until 8) {
            h[i] = h[i] xor v[i] xor v[i + 8]
        }
    }

    private fun g(v: LongArray, a: Int, b: Int, c: Int, d: Int, x: Long, y: Long) {
        v[a] = v[a] + v[b] + x
        v[d] = (v[d] xor v[a]).rotateRight(32)
        v[c] = v[c] + v[d]
        v[b] = (v[b] xor v[c]).rotateRight(24)
        v[a] = v[a] + v[b] + y
        v[d] = (v[d] xor v[a]).rotateRight(16)
        v[c] = v[c] + v[d]
        v[b] = (v[b] xor v[c]).rotateRight(63)
    }

    private fun Long.rotateRight(n: Int): Long = (this ushr n) or (this shl (64 - n))

    private fun readLongLE(data: ByteArray, offset: Int): Long {
        return (data[offset].toLong() and 0xFF) or
            ((data[offset + 1].toLong() and 0xFF) shl 8) or
            ((data[offset + 2].toLong() and 0xFF) shl 16) or
            ((data[offset + 3].toLong() and 0xFF) shl 24) or
            ((data[offset + 4].toLong() and 0xFF) shl 32) or
            ((data[offset + 5].toLong() and 0xFF) shl 40) or
            ((data[offset + 6].toLong() and 0xFF) shl 48) or
            ((data[offset + 7].toLong() and 0xFF) shl 56)
    }
}
