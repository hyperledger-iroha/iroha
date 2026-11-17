// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Represents the Norito header (NRT0 major 0).
 *
 * The minor version and the flag byte declare the payload layout ([layout]): minor `0` is the
 * current layout, with or without [COMPACT_LEN]; minor `1` is the dense layout, which has no
 * flags and no alignment padding (`norito.md`, "Dense layout (header minor 1)").
 */
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

    /** The payload layout this header declares through [minor] and [flags]. */
    @JvmField
    val layout: WireLayout

    init {
        require(schemaHash.size == 16) { "schemaHash must be 16 bytes" }
        require(payloadLength >= 0) { "payloadLength must be non-negative" }
        val normalizedMinor = minor and 0xFF
        require(normalizedMinor == MINOR_VERSION || normalizedMinor == MINOR_VERSION_DENSE) {
            "Unsupported Norito minor version: 0x${"%02x".format(normalizedMinor)}"
        }
        validateCompression(compression)
        validateFlags(flags)
        layout = WireLayout.fromHeader(normalizedMinor, flags)
        _schemaHash = schemaHash.copyOf()
        this.minor = normalizedMinor
    }

    val schemaHash: ByteArray get() = _schemaHash.copyOf()

    fun encode(): ByteArray {
        validateCompression(compression)
        validateFlags(flags)
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

        /** Compact (varint) length prefixes; the only layout flag Norito v1 defines. */
        const val COMPACT_LEN = 0x02

        /** Every other flag bit is reserved and rejected. */
        private const val SUPPORTED_FLAGS_MASK = COMPACT_LEN

        /** Minor version of the current layout; its layout flags are declared in the flag byte. */
        const val MINOR_VERSION = 0

        /** Minor version that declares the dense layout ([WireLayout.DENSE]); it admits no flags. */
        const val MINOR_VERSION_DENSE = 1

        const val COMPRESSION_NONE = 0
        const val COMPRESSION_ZSTD = 1

        const val HEADER_LENGTH = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1
        const val MAX_HEADER_PADDING = 64

        @JvmStatic
        fun validateFlags(flags: Int) {
            val normalizedFlags = flags and 0xFF
            val unsupportedFlags = normalizedFlags and SUPPORTED_FLAGS_MASK.inv()
            require(unsupportedFlags == 0) {
                "Unsupported Norito layout flags: 0x${"%02x".format(unsupportedFlags)}"
            }
        }

        private fun validateCompression(compression: Int) {
            require(compression == COMPRESSION_NONE || compression == COMPRESSION_ZSTD) {
                "Unsupported compression byte: $compression"
            }
        }

        /**
         * Reads a frame in the current layout (minor `0`).
         *
         * A dense frame (minor `1`) is refused: its flag byte is `0x00`, so a caller that built
         * a decoder from [flags] would misread it as the fixed-width layout. Callers that
         * accept dense frames name it through [decodeDeclared] or
         * [NoritoCodec.decode] with a [WireLayout].
         */
        @JvmStatic
        fun decode(data: ByteArray, expectedHash: ByteArray?): DecodeResult =
            copyPayload(decodeView(ByteBuffer.wrap(data), expectedHash))

        /** [decode] over a buffer view. */
        @JvmStatic
        fun decodeView(data: ByteBuffer, expectedHash: ByteArray?): DecodeView =
            readFrame(data, expectedHash, admitDense = false)

        /**
         * Reads a frame in whichever layout its header declares: minor `0` or the dense minor
         * `1`. The layout comes from the header alone ([layout]); payload bytes never select it.
         * The payload must then be decoded with that layout's code ([WireLayout.code]), not
         * with [flags].
         */
        @JvmStatic
        fun decodeDeclared(data: ByteArray, expectedHash: ByteArray?): DecodeResult =
            copyPayload(decodeDeclaredView(ByteBuffer.wrap(data), expectedHash))

        /** [decodeDeclared] over a buffer view. */
        @JvmStatic
        fun decodeDeclaredView(data: ByteBuffer, expectedHash: ByteArray?): DecodeView =
            readFrame(data, expectedHash, admitDense = true)

        private fun copyPayload(view: DecodeView): DecodeResult {
            val payloadView = view.payload
            val payload = ByteArray(payloadView.remaining())
            payloadView.get(payload)
            return DecodeResult(view.header, payload)
        }

        private fun readFrame(data: ByteBuffer, expectedHash: ByteArray?, admitDense: Boolean): DecodeView {
            val buffer = data.slice().order(ByteOrder.LITTLE_ENDIAN)
            require(buffer.remaining() >= HEADER_LENGTH) { "Insufficient data for Norito header" }

            val magic = ByteArray(4)
            buffer.get(magic)
            require(magic.contentEquals(MAGIC)) { "Invalid Norito magic" }

            val major = buffer.get().toInt() and 0xFF
            val minor = buffer.get().toInt() and 0xFF
            require(major == MAJOR_VERSION) { "Unsupported Norito version: $major.$minor" }
            require(minor == MINOR_VERSION || minor == MINOR_VERSION_DENSE) {
                "Unsupported Norito version: $major.$minor"
            }
            val dense = minor == MINOR_VERSION_DENSE
            require(!dense || admitDense) {
                "Unsupported Norito version: $major.$minor " +
                    "(a dense-layout frame; this reader accepts the current layout only)"
            }

            val schemaHash = ByteArray(16)
            buffer.get(schemaHash)
            if (expectedHash != null) {
                require(expectedHash.contentEquals(schemaHash)) { "Schema mismatch" }
            }

            val compression = buffer.get().toInt() and 0xFF
            validateCompression(compression)

            val payloadLength = buffer.getLong()
            require(payloadLength >= 0 && payloadLength <= Int.MAX_VALUE) {
                "Payload too large for Java implementation"
            }

            val checksum = buffer.getLong()

            val flags = buffer.get().toInt() and 0xFF
            validateFlags(flags)
            require(!dense || flags == 0) {
                "The dense Norito layout (minor 1) has no layout flags: 0x${"%02x".format(flags)}"
            }

            val intPayloadLength = payloadLength.toInt()
            var payloadOffset = HEADER_LENGTH
            val payloadLimit = buffer.limit()
            val payload: ByteBuffer

            if (compression == COMPRESSION_NONE) {
                val minEnd = payloadOffset + intPayloadLength
                require(minEnd <= payloadLimit) { "Length mismatch between header and payload" }
                val paddingLength = payloadLimit - minEnd
                require(!dense || paddingLength == 0) {
                    "A dense Norito frame has no header padding and no trailing data"
                }
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
