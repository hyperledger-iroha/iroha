// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer

/** 7-bit little-endian varint helpers. */
object Varint {
    @JvmStatic
    fun encode(value: Long): ByteArray {
        require(value >= 0) { "Varint cannot encode negative values" }
        val out = ByteArrayOutputStream()
        var remaining = value
        while (true) {
            val bits = (remaining and 0x7F).toInt()
            remaining = remaining ushr 7
            if (remaining != 0L) {
                out.write(bits or 0x80)
            } else {
                out.write(bits)
                break
            }
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun decode(data: ByteArray, offset: Int): DecodeResult {
        var result = 0L
        var shift = 0
        var index = offset
        while (true) {
            require(index < data.size) { "Unexpected end of data while decoding varint" }
            val b = data[index++].toInt() and 0xFF
            val chunk = b and 0x7F
            require(!(shift == 63 && chunk > 1)) { "Varint exceeds 64 bits" }
            result = result or (chunk.toLong() shl shift)
            if (b and 0x80 == 0) {
                require(!(shift > 0 && chunk == 0)) { "Varint is not canonically encoded" }
                break
            }
            shift += 7
            require(shift < 64) { "Varint exceeds 64 bits" }
        }
        return DecodeResult(result, index)
    }

    @JvmStatic
    fun decode(buffer: ByteBuffer): DecodeResult {
        var result = 0L
        var shift = 0
        val start = buffer.position()
        while (true) {
            require(buffer.hasRemaining()) { "Unexpected end of data while decoding varint" }
            val b = buffer.get().toInt() and 0xFF
            val chunk = b and 0x7F
            require(!(shift == 63 && chunk > 1)) { "Varint exceeds 64 bits" }
            result = result or (chunk.toLong() shl shift)
            if (b and 0x80 == 0) {
                require(!(shift > 0 && chunk == 0)) { "Varint is not canonically encoded" }
                break
            }
            shift += 7
            require(shift < 64) { "Varint exceeds 64 bits" }
        }
        return DecodeResult(result, start + (buffer.position() - start))
    }

    data class DecodeResult(
        @JvmField val value: Long,
        @JvmField val nextOffset: Int,
    ) {
        /** Record-style accessor for Java interop. */
        fun value(): Long = value

        /** Record-style accessor for Java interop. */
        fun nextOffset(): Int = nextOffset
    }
}
