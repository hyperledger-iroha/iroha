// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer
import java.nio.ByteOrder

/** Represents the Norito header (NRT0 major 0 with fixed v1 minor). */
class NoritoHeader(
    schemaHash: ByteArray,
    @JvmField val payloadLength: Int,
    @JvmField val checksum: Long,
    @JvmField val flags: Int,
    @JvmField val compression: Int,
    minor: Int = MINOR_VERSION,
) {
    private val _schemaHash: ByteArray

    @JvmField
    val minor: Int

    init {
        require(schemaHash.size == 16) { "schemaHash must be 16 bytes" }
        val normalizedMinor = minor and 0xFF
        require(normalizedMinor == MINOR_VERSION) {
            "Unsupported Norito minor version: 0x${"%02x".format(normalizedMinor)}"
        }
        _schemaHash = schemaHash.copyOf()
        this.minor = normalizedMinor
    }

    val schemaHash: ByteArray get() = _schemaHash.copyOf()

    fun encode(): ByteArray {
        val buffer = ByteBuffer.allocate(HEADER_LENGTH).order(ByteOrder.LITTLE_ENDIAN)
        buffer.put(MAGIC)
        buffer.put(MAJOR_VERSION.toByte())
        buffer.put((minor and 0xFF).toByte())
        buffer.put(_schemaHash)
        buffer.put(compression.toByte())
        buffer.putLong(payloadLength.toLong() and 0xFFFFFFFFL)
        buffer.putLong(checksum)
        buffer.put((flags and 0xFF).toByte())
        return buffer.array()
    }

    fun validateChecksum(payload: ByteArray) {
        val actual = CRC64.compute(payload)
        require(actual == checksum) {
            "Checksum mismatch: expected 0x${"%016x".format(checksum)} got 0x${"%016x".format(actual)}"
        }
    }

    fun validateChecksum(payload: ByteBuffer) {
        val actual = CRC64.compute(payload)
        require(actual == checksum) {
            "Checksum mismatch: expected 0x${"%016x".format(checksum)} got 0x${"%016x".format(actual)}"
        }
    }

    data class DecodeView(val header: NoritoHeader, val payload: ByteBuffer)

    data class DecodeResult(val header: NoritoHeader, val payload: ByteArray) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is DecodeResult) return false
            return header == other.header && payload.contentEquals(other.payload)
        }

        override fun hashCode(): Int {
            var result = header.hashCode()
            result = 31 * result + payload.contentHashCode()
            return result
        }
    }

    companion object {
        @JvmField
        val MAGIC = byteArrayOf('N'.code.toByte(), 'R'.code.toByte(), 'T'.code.toByte(), '0'.code.toByte())

        const val MAJOR_VERSION = 0

        const val PACKED_SEQ = 0x01
        const val COMPACT_LEN = 0x02
        const val PACKED_STRUCT = 0x04
        const val VARINT_OFFSETS = 0x08
        const val COMPACT_SEQ_LEN = 0x10
        const val FIELD_BITSET = 0x20

        private const val SUPPORTED_FLAGS_MASK =
            PACKED_SEQ or COMPACT_LEN or PACKED_STRUCT or VARINT_OFFSETS or COMPACT_SEQ_LEN or FIELD_BITSET

        /** Minor version is fixed for v1; layout flags are declared in the header flag byte. */
        const val MINOR_VERSION = 0

        const val COMPRESSION_NONE = 0
        const val COMPRESSION_ZSTD = 1

        const val HEADER_LENGTH = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1
        const val MAX_HEADER_PADDING = 64

        @JvmStatic
        fun decode(data: ByteArray, expectedHash: ByteArray?): DecodeResult {
            val view = decodeView(ByteBuffer.wrap(data), expectedHash)
            val payloadView = view.payload
            val payload = ByteArray(payloadView.remaining())
            payloadView.get(payload)
            return DecodeResult(view.header, payload)
        }

        @JvmStatic
        fun decodeView(data: ByteBuffer, expectedHash: ByteArray?): DecodeView {
            val buffer = data.slice().order(ByteOrder.LITTLE_ENDIAN)
            require(buffer.remaining() >= HEADER_LENGTH) { "Insufficient data for Norito header" }

            val magic = ByteArray(4)
            buffer.get(magic)
            require(magic.contentEquals(MAGIC)) { "Invalid Norito magic" }

            val major = buffer.get().toInt() and 0xFF
            val minor = buffer.get().toInt() and 0xFF
            require(major == MAJOR_VERSION) { "Unsupported Norito version: $major.$minor" }
            require(minor == MINOR_VERSION) { "Unsupported Norito version: $major.$minor" }

            val schemaHash = ByteArray(16)
            buffer.get(schemaHash)
            if (expectedHash != null) {
                require(expectedHash.contentEquals(schemaHash)) { "Schema mismatch" }
            }

            val compression = buffer.get().toInt() and 0xFF
            check(compression == COMPRESSION_NONE || compression == COMPRESSION_ZSTD) {
                "Unsupported compression byte: $compression"
            }

            val payloadLength = buffer.getLong()
            require(payloadLength <= Int.MAX_VALUE) { "Payload too large for Java implementation" }

            val checksum = buffer.getLong()

            val flags = buffer.get().toInt() and 0xFF
            val unsupportedFlags = flags and SUPPORTED_FLAGS_MASK.inv()
            require(unsupportedFlags == 0) {
                "Unsupported Norito layout flags: 0x${"%02x".format(unsupportedFlags)}"
            }

            val intPayloadLength = payloadLength.toInt()
            var payloadOffset = HEADER_LENGTH
            val payloadLimit = buffer.limit()
            val payload: ByteBuffer

            if (compression == COMPRESSION_NONE) {
                val minEnd = payloadOffset + intPayloadLength
                require(minEnd <= payloadLimit) { "Length mismatch between header and payload" }
                val paddingLength = payloadLimit - minEnd
                require(paddingLength <= MAX_HEADER_PADDING) { "Trailing data after Norito payload" }
                if (paddingLength > 0) {
                    for (i in 0 until paddingLength) {
                        require(buffer.get(payloadOffset + i).toInt() == 0) {
                            "Non-zero Norito header padding"
                        }
                    }
                }
                payloadOffset += paddingLength
                val payloadEnd = payloadOffset + intPayloadLength
                require(payloadEnd == payloadLimit) { "Trailing data after Norito payload" }
                val dup = buffer.duplicate()
                dup.position(payloadOffset)
                dup.limit(payloadEnd)
                payload = dup.slice().order(ByteOrder.LITTLE_ENDIAN)
            } else {
                require(payloadOffset < payloadLimit) { "Missing compressed payload bytes" }
                val dup = buffer.duplicate()
                dup.position(payloadOffset)
                dup.limit(payloadLimit)
                payload = dup.slice().order(ByteOrder.LITTLE_ENDIAN)
            }

            val header = NoritoHeader(schemaHash, intPayloadLength, checksum, flags, compression, minor)
            return DecodeView(header, payload)
        }
    }
}
