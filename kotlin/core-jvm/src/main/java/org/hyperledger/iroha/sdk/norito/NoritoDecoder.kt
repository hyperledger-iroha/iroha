// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Reader for Norito payloads.
 *
 * [flags] is the layout code of the payload being read: a minor-0 header flag byte, or
 * [WireLayout.DENSE]'s code for the dense layout. The owner of the bytes names it (from a frame
 * header or explicitly for a bare payload); it is never inferred from the payload.
 */
class NoritoDecoder(
    private val buffer: ByteBuffer,
    @JvmField val flags: Int,
) {
    constructor(data: ByteArray, flags: Int) : this(
        buffer = ByteBuffer.wrap(data.copyOf()).order(ByteOrder.LITTLE_ENDIAN),
        flags = flags,
    )

    /** Creates a decoder for a bare payload in `layout`. */
    constructor(data: ByteArray, layout: WireLayout) : this(data, layout.code)

    /** The layout this decoder reads, selected by [flags]. */
    @JvmField
    val layout: WireLayout = WireLayout.fromCode(flags)

    fun compactLenActive(): Boolean = (flags and NoritoHeader.COMPACT_LEN) != 0

    /** Whether this decoder reads the dense layout (`norito.md`, "Dense layout"). */
    fun denseLayoutActive(): Boolean = layout == WireLayout.DENSE

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

    /**
     * Reads a per-value length prefix: a varint when `compact`, otherwise a fixed `u64`.
     *
     * The dense layout has no fixed-width length, so a fixed-width read is refused there;
     * sequence, map and set counts go through [readCount].
     */
    fun readLength(compact: Boolean): Long {
        check(compact || !denseLayoutActive()) {
            "The dense Norito layout has no fixed-width lengths; read counts with readCount"
        }
        val length = if (compact) readVarint() else readUInt(64)
        require(length in 0..Int.MAX_VALUE.toLong()) {
            "Norito length exceeds the supported JVM range"
        }
        return length
    }

    /**
     * Reads a sequence, map or set count: a fixed little-endian `u64` in the minor-0 layouts
     * and a shortest-form varint in the dense layout.
     */
    fun readCount(): Long = readLength(denseLayoutActive())

    /**
     * Reads an enum discriminant: a little-endian `u32` in the minor-0 layouts and a
     * shortest-form varint of at most `u32::MAX` in the dense layout.
     */
    fun readEnumTag(): Long {
        if (!denseLayoutActive()) return readUInt(32)
        val tag = readVarint()
        require(tag in 0L..MAX_ENUM_TAG) { "Enum tag exceeds u32" }
        return tag
    }

    fun readVarint(): Long {
        val result = Varint.decode(buffer)
        return result.value
    }

    private fun ensure(count: Int) {
        require(buffer.remaining() >= count) { "Unexpected end of data" }
    }
}
