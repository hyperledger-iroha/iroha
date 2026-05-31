// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.crypto

/**
 * Pure Kotlin implementation of the BLAKE3 hash function.
 *
 * Supports inputs up to the largest Solana AccountLtHash preimage used by the Kotlin SDK,
 * and exposes BLAKE3 XOF output for Agave-compatible 2048-byte account contributions.
 *
 * Reference: [BLAKE3 Specification](https://github.com/BLAKE3-team/BLAKE3-specs)
 */
object Blake3 {

    private const val BLOCK_LEN = 64
    private const val CHUNK_LEN = 1024
    private const val OUT_LEN = 32
    private const val MAX_INPUT_LEN = 8 + 65_536 + 1 + 32 + 32

    private const val CHUNK_START = 1
    private const val CHUNK_END = 2
    private const val PARENT = 4
    private const val ROOT = 8

    private val IV = intArrayOf(
        0x6A09E667, -0x4498517B, 0x3C6EF372, -0x5AB00AC6,
        0x510E527F, -0x64FA9774, 0x1F83D9AB, 0x5BE0CD19,
    )

    private val MSG_PERMUTATION = intArrayOf(
        2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
    )

    /**
     * Computes the BLAKE3 hash of the given input.
     *
     * @param input the data to hash
     * @return the 32-byte BLAKE3 hash
     * @throws IllegalArgumentException if input exceeds the SDK helper limit
     */
    @JvmStatic
    fun hash(input: ByteArray): ByteArray = derive(input, OUT_LEN)

    /**
     * Computes BLAKE3 XOF output for the given input.
     *
     * @param input the data to hash
     * @param outputLength number of output bytes to derive
     * @return BLAKE3 XOF bytes
     * @throws IllegalArgumentException if input or output length exceeds SDK helper bounds
     */
    @JvmStatic
    fun derive(input: ByteArray, outputLength: Int): ByteArray {
        require(input.size <= MAX_INPUT_LEN) {
            "Input too large for Blake3 helper: ${input.size} bytes (max $MAX_INPUT_LEN)"
        }
        require(outputLength >= 0) { "outputLength must not be negative" }
        val rootOutput = rootOutput(input)
        val output = ByteArray(outputLength)
        var cursor = 0
        var outputBlockCounter = 0L
        while (cursor < outputLength) {
            val words = rootOutput.rootWords(outputBlockCounter)
            for (word in words) {
                if (cursor >= outputLength) break
                output[cursor++] = word.toByte()
                if (cursor >= outputLength) break
                output[cursor++] = (word ushr 8).toByte()
                if (cursor >= outputLength) break
                output[cursor++] = (word ushr 16).toByte()
                if (cursor >= outputLength) break
                output[cursor++] = (word ushr 24).toByte()
            }
            outputBlockCounter += 1
        }
        return output
    }

    private fun rootOutput(input: ByteArray): Output {
        val chunkCount = maxOf(1, (input.size + CHUNK_LEN - 1) / CHUNK_LEN)
        return subtreeOutput(input, 0, chunkCount)
    }

    private fun subtreeOutput(input: ByteArray, chunkIndex: Int, chunkCount: Int): Output {
        if (chunkCount == 1) {
            val offset = chunkIndex * CHUNK_LEN
            val length = minOf(CHUNK_LEN, maxOf(0, input.size - offset))
            return chunkOutput(input, offset, length, chunkIndex.toLong())
        }
        val leftCount = leftSubtreeChunkCount(chunkCount)
        val left = subtreeOutput(input, chunkIndex, leftCount).chainingValue()
        val right = subtreeOutput(input, chunkIndex + leftCount, chunkCount - leftCount).chainingValue()
        return parentOutput(left, right)
    }

    private fun leftSubtreeChunkCount(chunkCount: Int): Int {
        var power = 1
        while (power * 2 < chunkCount) {
            power *= 2
        }
        return power
    }

    private fun chunkOutput(input: ByteArray, offset: Int, length: Int, chunkCounter: Long): Output {
        val cv = IV.copyOf()
        val numBlocks = maxOf(1, (length + BLOCK_LEN - 1) / BLOCK_LEN)

        for (i in 0 until numBlocks) {
            val blockStart = offset + i * BLOCK_LEN
            val blockLen = minOf(BLOCK_LEN, maxOf(0, length - i * BLOCK_LEN))

            val blockWords = parseBlockWords(input, blockStart)

            var flags = 0
            if (i == 0) flags = flags or CHUNK_START
            if (i == numBlocks - 1) {
                return Output(cv.copyOf(), blockWords, chunkCounter, blockLen, flags or CHUNK_END)
            }
            val state = compress(cv, blockWords, chunkCounter, blockLen, flags)
            for (j in 0 until 8) {
                cv[j] = state[j] xor state[j + 8]
            }
        }
        error("Unreachable")
    }

    private fun parentOutput(left: IntArray, right: IntArray): Output {
        val blockWords = IntArray(16)
        for (i in 0 until 8) {
            blockWords[i] = left[i]
            blockWords[i + 8] = right[i]
        }
        return Output(IV.copyOf(), blockWords, 0L, BLOCK_LEN, PARENT)
    }

    private fun parseBlockWords(input: ByteArray, blockStart: Int): IntArray {
        val words = IntArray(16)
        for (j in 0 until 16) {
            val offset = blockStart + j * 4
            val b0 = if (offset < input.size) (input[offset].toInt() and 0xFF) else 0
            val b1 = if (offset + 1 < input.size) (input[offset + 1].toInt() and 0xFF) else 0
            val b2 = if (offset + 2 < input.size) (input[offset + 2].toInt() and 0xFF) else 0
            val b3 = if (offset + 3 < input.size) (input[offset + 3].toInt() and 0xFF) else 0
            words[j] = b0 or (b1 shl 8) or (b2 shl 16) or (b3 shl 24)
        }
        return words
    }

    private fun compress(cv: IntArray, blockWords: IntArray, counter: Long, blockLen: Int, flags: Int): IntArray {
        val state = intArrayOf(
            cv[0], cv[1], cv[2], cv[3],
            cv[4], cv[5], cv[6], cv[7],
            IV[0], IV[1], IV[2], IV[3],
            (counter and 0xFFFFFFFFL).toInt(), ((counter ushr 32) and 0xFFFFFFFFL).toInt(),
            blockLen, flags,
        )
        var msg = blockWords.copyOf()

        for (round in 0 until 7) {
            g(state, 0, 4, 8, 12, msg[0], msg[1])
            g(state, 1, 5, 9, 13, msg[2], msg[3])
            g(state, 2, 6, 10, 14, msg[4], msg[5])
            g(state, 3, 7, 11, 15, msg[6], msg[7])
            g(state, 0, 5, 10, 15, msg[8], msg[9])
            g(state, 1, 6, 11, 12, msg[10], msg[11])
            g(state, 2, 7, 8, 13, msg[12], msg[13])
            g(state, 3, 4, 9, 14, msg[14], msg[15])

            if (round < 6) {
                msg = permute(msg)
            }
        }

        return state
    }

    private fun g(state: IntArray, a: Int, b: Int, c: Int, d: Int, mx: Int, my: Int) {
        state[a] = state[a] + state[b] + mx
        state[d] = Integer.rotateRight(state[d] xor state[a], 16)
        state[c] = state[c] + state[d]
        state[b] = Integer.rotateRight(state[b] xor state[c], 12)
        state[a] = state[a] + state[b] + my
        state[d] = Integer.rotateRight(state[d] xor state[a], 8)
        state[c] = state[c] + state[d]
        state[b] = Integer.rotateRight(state[b] xor state[c], 7)
    }

    private fun permute(msg: IntArray): IntArray = IntArray(16) { msg[MSG_PERMUTATION[it]] }

    private class Output(
        private val inputChainingValue: IntArray,
        private val blockWords: IntArray,
        private val counter: Long,
        private val blockLen: Int,
        private val flags: Int,
    ) {
        fun chainingValue(): IntArray {
            val state = compress(inputChainingValue, blockWords, counter, blockLen, flags)
            return IntArray(8) { state[it] xor state[it + 8] }
        }

        fun rootWords(outputBlockCounter: Long): IntArray {
            val state = compress(inputChainingValue, blockWords, outputBlockCounter, blockLen, flags or ROOT)
            val words = IntArray(16)
            for (index in 0 until 8) {
                words[index] = state[index] xor state[index + 8]
                words[index + 8] = state[index + 8] xor inputChainingValue[index]
            }
            return words
        }
    }
}
