// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Reader for Norito payloads. */
class NoritoDecoder(
    private val buffer: ByteBuffer,
    @JvmField val flags: Int,
    @JvmField val flagsHint: Int = NoritoHeader.MINOR_VERSION,
) {
    constructor(data: ByteArray, flags: Int) : this(data, flags, NoritoHeader.MINOR_VERSION)

    constructor(data: ByteArray, flags: Int, flagsHint: Int) : this(
        buffer = ByteBuffer.wrap(data.copyOf()).order(ByteOrder.LITTLE_ENDIAN),
        flags = flags,
        flagsHint = flagsHint,
    )

    fun compactLenActive(): Boolean = (flags and NoritoHeader.COMPACT_LEN) != 0

    fun remaining(): Int = buffer.remaining()

    fun readBytes(count: Int): ByteArray {
        ensure(count)
        val out = ByteArray(count)
        buffer.get(out)
        return out
    }

    fun readByte(): Int {
        ensure(1)
        return buffer.get().toInt() and 0xFF
    }

    fun readUInt(bits: Int): Long {
        ensure(bits / 8)
        return when (bits) {
            8 -> buffer.get().toLong() and 0xFFL
            16 -> buffer.getShort().toLong() and 0xFFFFL
            32 -> buffer.getInt().toLong() and 0xFFFFFFFFL
            64 -> buffer.getLong()
            else -> throw IllegalArgumentException("Unsupported integer size: $bits")
        }
    }

    fun readInt(bits: Int): Long {
        ensure(bits / 8)
        return when (bits) {
            8 -> buffer.get().toLong()
            16 -> buffer.getShort().toLong()
            32 -> buffer.getInt().toLong()
            64 -> buffer.getLong()
            else -> throw IllegalArgumentException("Unsupported integer size: $bits")
        }
    }

    fun readLength(compact: Boolean): Long {
        if (compact) return readVarint()
        return readUInt(64)
    }

    fun readVarint(): Long {
        val result = Varint.decode(buffer)
        return result.value
    }

    private fun ensure(count: Int) {
        require(buffer.remaining() >= count) { "Unexpected end of data" }
    }
}
