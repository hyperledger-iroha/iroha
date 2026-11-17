// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Low-level helper for writing Norito payloads.
 *
 * [flags] is the layout code of the payload being written: a minor-0 header flag byte, or
 * [WireLayout.DENSE]'s code for the dense layout. Child encoders inherit it.
 */
class NoritoEncoder(@JvmField val flags: Int) {

    /** Creates an encoder for a payload in `layout`. */
    constructor(layout: WireLayout) : this(layout.code)

    /** The layout this encoder writes, selected by [flags]. */
    @JvmField
    val layout: WireLayout = WireLayout.fromCode(flags)

    private val buffer = ByteArrayOutputStream()

    /** Whether this encoder writes the dense layout (`norito.md`, "Dense layout"). */
    fun denseLayoutActive(): Boolean = layout == WireLayout.DENSE

    fun writeByte(value: Int) {
        buffer.write(value and 0xFF)
    }

    fun writeBytes(data: ByteArray) {
        buffer.write(data, 0, data.size)
    }

    fun writeUInt(value: Long, bits: Int) {
        val bytes = bits / 8
        val bb = ByteBuffer.allocate(bytes).order(ByteOrder.LITTLE_ENDIAN)
        when (bits) {
            64 -> bb.putLong(value)
            32 -> bb.putInt(value.toInt())
            16 -> bb.putShort(value.toShort())
            8 -> bb.put(value.toByte())
            else -> throw IllegalArgumentException("Unsupported integer size: $bits")
        }
        writeBytes(bb.array())
    }

    fun writeInt(value: Long, bits: Int) {
        writeUInt(value, bits)
    }

    /**
     * Writes a per-value length prefix: a varint when `compact`, otherwise a fixed `u64`.
     *
     * The dense layout has no fixed-width length, so a fixed-width write is refused there;
     * sequence, map and set counts go through [writeCount].
     */
    fun writeLength(value: Long, compact: Boolean) {
        check(compact || !denseLayoutActive()) {
            "The dense Norito layout has no fixed-width lengths; write counts with writeCount"
        }
        if (compact) {
            writeBytes(Varint.encode(value))
        } else {
            writeUInt(value, 64)
        }
    }

    /**
     * Writes a sequence, map or set count: a fixed little-endian `u64` in the minor-0 layouts
     * and a shortest-form varint in the dense layout.
     */
    fun writeCount(value: Long) {
        if (denseLayoutActive()) {
            writeBytes(Varint.encode(value))
        } else {
            writeUInt(value, 64)
        }
    }

    /**
     * Writes an enum discriminant: a little-endian `u32` in the minor-0 layouts and a
     * shortest-form varint of the same value in the dense layout.
     */
    fun writeEnumTag(tag: Long) {
        require(tag in 0L..MAX_ENUM_TAG) { "Enum tag must fit in u32: $tag" }
        if (denseLayoutActive()) {
            writeBytes(Varint.encode(tag))
        } else {
            writeUInt(tag, 32)
        }
    }

    fun childEncoder(): NoritoEncoder = NoritoEncoder(flags)

    fun toByteArray(): ByteArray = buffer.toByteArray()

    fun append(data: ByteArray) {
        writeBytes(data)
    }
}

/** Largest enum discriminant: tags are `u32` values in every layout. */
internal const val MAX_ENUM_TAG = 0xFFFF_FFFFL
