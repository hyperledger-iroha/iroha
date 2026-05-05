// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

/** Baseline streaming codec helpers aligned with the Rust Norito implementation. */
object NoritoStreamingCodec {

    const val BLOCK_SIZE = 8
    const val BLOCK_PIXELS = BLOCK_SIZE * BLOCK_SIZE
    const val RLE_EOB = 0xFF
    const val MAX_ZERO_RUN = 254

    private val ZIG_ZAG = intArrayOf(
        0, 1, 8, 16, 9, 2, 3, 10,
        17, 24, 32, 25, 18, 11, 4, 5,
        12, 19, 26, 33, 40, 48, 41, 34,
        27, 20, 13, 6, 7, 14, 21, 28,
        35, 42, 49, 56, 57, 50, 43, 36,
        29, 22, 15, 23, 30, 37, 44, 51,
        58, 59, 52, 45, 38, 31, 39, 46,
        53, 60, 61, 54, 47, 55, 62, 63,
    )

    sealed interface CodecError {
        val blockIndex: Int

        data class RleOverflow(override val blockIndex: Int) : CodecError
        data class TruncatedBlock(override val blockIndex: Int) : CodecError
        data class MissingEndOfBlock(override val blockIndex: Int) : CodecError
    }

    class CodecException(
        @JvmField val error: CodecError,
    ) : RuntimeException(error.toString())

    data class BlockDecodeResult(
        val coeffs: ShortArray,
        @JvmField val offset: Int,
        @JvmField val prevDc: Short,
    ) {
        init {
            require(coeffs.size == BLOCK_PIXELS) { "coeffs must be 64 elements" }
        }

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is BlockDecodeResult) return false
            return coeffs.contentEquals(other.coeffs)
                && offset == other.offset
                && prevDc == other.prevDc
        }

        override fun hashCode(): Int {
            var result = coeffs.contentHashCode()
            result = 31 * result + offset
            result = 31 * result + prevDc.hashCode()
            return result
        }
    }

    /**
     * Decode a single run-length encoded block.
     *
     * @param bytes source payload
     * @param offset current byte offset
     * @param prevDc previous DC coefficient
     * @param blockIndex block index for error reporting
     * @return decoded coefficients and updated cursor state
     */
    @JvmStatic
    fun decodeBlockRle(bytes: ByteArray, offset: Int, prevDc: Short, blockIndex: Int): BlockDecodeResult {
        require(offset in 0..bytes.size) { "offset out of range" }
        if (offset + 2 > bytes.size) {
            throw CodecException(CodecError.TruncatedBlock(blockIndex))
        }
        val coeffs = ShortArray(BLOCK_PIXELS)
        var currentOffset = offset
        val dcDiff = readI16Le(bytes, currentOffset)
        currentOffset += 2
        val dc = (prevDc + dcDiff).toShort()
        coeffs[0] = dc
        var currentPrevDc = dc

        var pos = 1
        var finished = false
        while (pos < BLOCK_PIXELS) {
            if (currentOffset + 3 > bytes.size) {
                throw CodecException(CodecError.TruncatedBlock(blockIndex))
            }
            val run = bytes[currentOffset].toInt() and 0xFF
            val value = readI16Le(bytes, currentOffset + 1)
            currentOffset += 3
            if (run == RLE_EOB) {
                finished = true
                break
            }
            val advance = run
            if (pos + advance >= BLOCK_PIXELS) {
                throw CodecException(CodecError.RleOverflow(blockIndex))
            }
            pos += advance
            coeffs[ZIG_ZAG[pos]] = value
            pos += 1
        }

        if (!finished) {
            if (pos < BLOCK_PIXELS) {
                throw CodecException(CodecError.TruncatedBlock(blockIndex))
            }
            if (currentOffset + 3 <= bytes.size && (bytes[currentOffset].toInt() and 0xFF) == RLE_EOB) {
                currentOffset += 3
                return BlockDecodeResult(coeffs, currentOffset, currentPrevDc)
            }
            throw CodecException(CodecError.MissingEndOfBlock(blockIndex))
        }
        return BlockDecodeResult(coeffs, currentOffset, currentPrevDc)
    }

    private fun readI16Le(bytes: ByteArray, offset: Int): Short {
        val lo = bytes[offset].toInt() and 0xFF
        val hi = bytes[offset + 1].toInt() and 0xFF
        return (lo or (hi shl 8)).toShort()
    }
}
