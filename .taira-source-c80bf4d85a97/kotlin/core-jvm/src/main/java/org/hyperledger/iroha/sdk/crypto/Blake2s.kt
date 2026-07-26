package org.hyperledger.iroha.sdk.crypto

/** Minimal BLAKE2s implementation supporting keyed hashing. */
object Blake2s {

    private const val BLOCK_BYTES = 64
    private const val ROUNDS = 10

    private val IV = intArrayOf(
        0x6a09e667,
        -0x4498517b,  // 0xbb67ae85
        0x3c6ef372,
        -0x5ab00ac6,  // 0xa54ff53a
        0x510e527f,
        -0x64fa9774,  // 0x9b05688c
        0x1f83d9ab,
        0x5be0cd19,
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
    )

    @JvmStatic
    fun digest(message: ByteArray, key: ByteArray, outLen: Int): ByteArray {
        require(outLen in 1..32) { "BLAKE2s output length must be between 1 and 32 bytes" }
        require(key.size <= 32) { "BLAKE2s key must be at most 32 bytes" }

        val h = IV.copyOf()
        val param = outLen or (key.size shl 8) or (1 shl 16) or (1 shl 24)
        h[0] = h[0] xor param

        var t0 = 0
        var t1 = 0

        if (key.isNotEmpty()) {
            val block = ByteArray(BLOCK_BYTES)
            key.copyInto(block)
            val sum = (t0.toLong() and 0xFFFFFFFFL) + BLOCK_BYTES
            t0 = sum.toInt()
            if ((sum ushr 32) != 0L) {
                t1++
            }
            compress(h, block, 0, t0, t1, message.isEmpty())
        }

        val total = message.size
        if (total > 0) {
            var offset = 0
            val fullBlocks = total / BLOCK_BYTES
            val remainder = total % BLOCK_BYTES

            for (i in 0 until fullBlocks) {
                offset += BLOCK_BYTES
                val sum = (t0.toLong() and 0xFFFFFFFFL) + BLOCK_BYTES
                t0 = sum.toInt()
                if ((sum ushr 32) != 0L) {
                    t1++
                }
                val isLastFull = (i == fullBlocks - 1) && remainder == 0
                compress(h, message, offset - BLOCK_BYTES, t0, t1, isLastFull)
            }

            if (remainder > 0) {
                val block = ByteArray(BLOCK_BYTES)
                message.copyInto(block, destinationOffset = 0, startIndex = total - remainder, endIndex = total)
                val sum = (t0.toLong() and 0xFFFFFFFFL) + remainder
                t0 = sum.toInt()
                if ((sum ushr 32) != 0L) {
                    t1++
                }
                compress(h, block, 0, t0, t1, last = true)
            }
        } else if (key.isEmpty()) {
            val block = ByteArray(BLOCK_BYTES)
            compress(h, block, 0, 0, 0, last = true)
        }

        return toOutput(h, outLen)
    }

    private fun compress(
        h: IntArray,
        block: ByteArray,
        blockOffset: Int,
        t0: Int,
        t1: Int,
        last: Boolean,
    ) {
        val m = IntArray(16) { readIntLE(block, blockOffset + it * 4) }

        val v = IntArray(16)
        h.copyInto(v, destinationOffset = 0, startIndex = 0, endIndex = 8)
        IV.copyInto(v, destinationOffset = 8, startIndex = 0, endIndex = 8)
        v[12] = v[12] xor t0
        v[13] = v[13] xor t1
        if (last) {
            v[14] = v[14] xor -1 // 0xFFFFFFFF
        }

        for (round in 0 until ROUNDS) {
            val s = SIGMA[round]
            g(v, 0, 4, 8, 12, m[s[0].toInt() and 0xFF], m[s[1].toInt() and 0xFF])
            g(v, 1, 5, 9, 13, m[s[2].toInt() and 0xFF], m[s[3].toInt() and 0xFF])
            g(v, 2, 6, 10, 14, m[s[4].toInt() and 0xFF], m[s[5].toInt() and 0xFF])
            g(v, 3, 7, 11, 15, m[s[6].toInt() and 0xFF], m[s[7].toInt() and 0xFF])
            g(v, 0, 5, 10, 15, m[s[8].toInt() and 0xFF], m[s[9].toInt() and 0xFF])
            g(v, 1, 6, 11, 12, m[s[10].toInt() and 0xFF], m[s[11].toInt() and 0xFF])
            g(v, 2, 7, 8, 13, m[s[12].toInt() and 0xFF], m[s[13].toInt() and 0xFF])
            g(v, 3, 4, 9, 14, m[s[14].toInt() and 0xFF], m[s[15].toInt() and 0xFF])
        }

        for (i in 0 until 8) {
            h[i] = h[i] xor v[i] xor v[i + 8]
        }
    }

    private fun g(v: IntArray, a: Int, b: Int, c: Int, d: Int, x: Int, y: Int) {
        v[a] = v[a] + v[b] + x
        v[d] = Integer.rotateRight(v[d] xor v[a], 16)
        v[c] = v[c] + v[d]
        v[b] = Integer.rotateRight(v[b] xor v[c], 12)
        v[a] = v[a] + v[b] + y
        v[d] = Integer.rotateRight(v[d] xor v[a], 8)
        v[c] = v[c] + v[d]
        v[b] = Integer.rotateRight(v[b] xor v[c], 7)
    }

    private fun readIntLE(data: ByteArray, offset: Int): Int {
        return (data[offset].toInt() and 0xFF) or
            ((data[offset + 1].toInt() and 0xFF) shl 8) or
            ((data[offset + 2].toInt() and 0xFF) shl 16) or
            ((data[offset + 3].toInt() and 0xFF) shl 24)
    }

    private fun toOutput(h: IntArray, outLen: Int): ByteArray {
        val out = ByteArray(outLen)
        var idx = 0
        for (value in h) {
            var v = value
            for (j in 0 until 4) {
                if (idx >= outLen) break
                out[idx++] = (v and 0xFF).toByte()
                v = v ushr 8
            }
            if (idx >= outLen) break
        }
        return out
    }
}
