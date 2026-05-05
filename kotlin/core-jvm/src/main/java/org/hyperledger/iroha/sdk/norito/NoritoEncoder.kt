// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.io.ByteArrayOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Low-level helper for writing Norito payloads. */
class NoritoEncoder(@JvmField val flags: Int) {

    private val buffer = ByteArrayOutputStream()

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

    fun writeLength(value: Long, compact: Boolean) {
        if (compact) {
            writeBytes(Varint.encode(value))
        } else {
            writeUInt(value, 64)
        }
    }

    fun childEncoder(): NoritoEncoder = NoritoEncoder(flags)

    fun toByteArray(): ByteArray = buffer.toByteArray()

    fun append(data: ByteArray) {
        writeBytes(data)
    }
}
