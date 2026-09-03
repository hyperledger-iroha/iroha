// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.io.Closeable
import java.security.MessageDigest

/**
 * Short-APDU transport for one already selected, access-controlled secure-element applet.
 *
 * The transport only moves ABI-23 lifecycle frames. Authority is admitted separately by
 * [KagemushaDeviceLifecycleBridgeV1], which requires the exact complete IKGMJCP1 capability
 * frame before exposing an available bridge.
 */
internal class KagemushaSecureElementApduEndpointV1(
    private val channel: Channel,
) : KagemushaDeviceLifecycleBridgeV1.Endpoint, Closeable {
    internal fun interface Channel : Closeable {
        /** Return response data followed by the two-byte ISO 7816 status word. */
        fun transmit(command: ByteArray): ByteArray

        override fun close() = Unit
    }

    private val lock = Any()

    override fun capabilities(): ByteArray = synchronized(lock) {
        exchange(
            shortCommand(INS_CAPABILITIES, expectedLength = CAPABILITY_BYTES),
            "capabilities",
        ).also {
            require(it.size == CAPABILITY_BYTES) {
                "secure-element capability response must contain exactly $CAPABILITY_BYTES bytes"
            }
        }
    }

    override fun execute(command: ByteArray): ByteArray = synchronized(lock) {
        require(command.size in MINIMUM_COMMAND_BYTES..MAXIMUM_COMMAND_BYTES) {
            "secure-element command is outside the ABI-23 bound"
        }
        val commandDigest = sha256(command)
        try {
            val begin = ByteArray(BEGIN_BYTES)
            writeU32Le(begin, 0, command.size)
            commandDigest.copyInto(begin, LENGTH_BYTES)
            requireEmptySuccess(exchange(shortCommand(INS_BEGIN_COMMAND, data = begin), "begin"), "begin")

            command.asList().chunked(CHUNK_BYTES).forEachIndexed { index, chunk ->
                val bytes = chunk.toByteArray()
                try {
                    requireEmptySuccess(
                        exchange(
                            shortCommand(
                                INS_WRITE_COMMAND,
                                p1 = index ushr 8,
                                p2 = index,
                                data = bytes,
                            ),
                            "write chunk $index",
                        ),
                        "write chunk $index",
                    )
                } finally {
                    bytes.fill(0)
                }
            }

            val metadata = exchange(
                shortCommand(INS_COMMIT_COMMAND, expectedLength = RESPONSE_METADATA_BYTES),
                "commit",
            )
            require(metadata.size == RESPONSE_METADATA_BYTES) {
                "secure-element response metadata must contain exactly $RESPONSE_METADATA_BYTES bytes"
            }
            val responseLength = readU32Le(metadata, 0)
            require(responseLength in MINIMUM_RESPONSE_BYTES..MAXIMUM_RESPONSE_BYTES) {
                "secure-element response is outside the ABI-23 bound"
            }
            val expectedDigest = metadata.copyOfRange(LENGTH_BYTES, RESPONSE_METADATA_BYTES)
            val response = ByteArray(responseLength)
            try {
                var offset = 0
                var index = 0
                while (offset < response.size) {
                    val count = minOf(CHUNK_BYTES, response.size - offset)
                    val chunk = exchange(
                        shortCommand(
                            INS_READ_RESPONSE,
                            p1 = index ushr 8,
                            p2 = index,
                            expectedLength = count,
                        ),
                        "read chunk $index",
                    )
                    require(chunk.size == count) {
                        "secure-element response chunk $index has the wrong length"
                    }
                    chunk.copyInto(response, offset)
                    chunk.fill(0)
                    offset += count
                    index += 1
                }
                require(MessageDigest.isEqual(expectedDigest, sha256(response))) {
                    "secure-element response digest mismatch"
                }
                return@synchronized response
            } catch (error: Throwable) {
                response.fill(0)
                throw error
            } finally {
                expectedDigest.fill(0)
                metadata.fill(0)
            }
        } catch (error: Throwable) {
            abortBestEffort()
            throw error
        } finally {
            commandDigest.fill(0)
        }
    }

    override fun close() = synchronized(lock) {
        abortBestEffort()
        channel.close()
    }

    private fun exchange(command: ByteArray, label: String): ByteArray {
        val raw = try {
            channel.transmit(command)
        } finally {
            command.fill(0)
        }
        require(raw.size >= STATUS_BYTES) { "secure-element $label response omitted its status word" }
        val status = ((raw[raw.lastIndex - 1].toInt() and 0xff) shl 8) or
            (raw[raw.lastIndex].toInt() and 0xff)
        if (status != SUCCESS_STATUS) {
            raw.fill(0)
            throw IllegalStateException("secure-element $label failed with status %04x".format(status))
        }
        return raw.copyOf(raw.size - STATUS_BYTES).also { raw.fill(0) }
    }

    private fun requireEmptySuccess(response: ByteArray, label: String) {
        try {
            require(response.isEmpty()) { "secure-element $label returned unexpected bytes" }
        } finally {
            response.fill(0)
        }
    }

    private fun abortBestEffort() {
        val command = byteArrayOf(CLA.toByte(), INS_ABORT_TRANSPORT.toByte(), 0, 0)
        runCatching { channel.transmit(command) }
            .getOrNull()
            ?.fill(0)
        command.fill(0)
    }

    private fun shortCommand(
        instruction: Int,
        p1: Int = 0,
        p2: Int = 0,
        data: ByteArray = ByteArray(0),
        expectedLength: Int? = null,
    ): ByteArray {
        require(data.size <= 255)
        require(expectedLength == null || expectedLength in 1..256)
        require(data.isEmpty() || expectedLength == null)
        return if (data.isNotEmpty()) {
            byteArrayOf(
                CLA.toByte(), instruction.toByte(), p1.toByte(), p2.toByte(), data.size.toByte(),
            ) + data
        } else if (expectedLength != null) {
            byteArrayOf(
                CLA.toByte(), instruction.toByte(), p1.toByte(), p2.toByte(),
                (expectedLength and 0xff).toByte(),
            )
        } else {
            byteArrayOf(CLA.toByte(), instruction.toByte(), p1.toByte(), p2.toByte())
        }
    }

    companion object {
        private const val CLA = 0x80
        private const val INS_CAPABILITIES = 0x11
        private const val INS_BEGIN_COMMAND = 0x12
        private const val INS_WRITE_COMMAND = 0x13
        private const val INS_COMMIT_COMMAND = 0x14
        private const val INS_READ_RESPONSE = 0x15
        private const val INS_ABORT_TRANSPORT = 0x16
        private const val CHUNK_BYTES = 224
        private const val LENGTH_BYTES = 4
        private const val DIGEST_BYTES = 32
        private const val BEGIN_BYTES = LENGTH_BYTES + DIGEST_BYTES
        private const val RESPONSE_METADATA_BYTES = LENGTH_BYTES + DIGEST_BYTES
        private const val CAPABILITY_BYTES = 96
        private const val MINIMUM_COMMAND_BYTES = 80
        private const val MAXIMUM_COMMAND_BYTES = 80 + 64 * 1024
        private const val MINIMUM_RESPONSE_BYTES = 116
        private const val MAXIMUM_RESPONSE_BYTES = 116 + 64 * 1024 + 8 * 1024
        private const val STATUS_BYTES = 2
        private const val SUCCESS_STATUS = 0x9000

        private fun sha256(bytes: ByteArray): ByteArray =
            MessageDigest.getInstance("SHA-256").digest(bytes)

        private fun writeU32Le(target: ByteArray, offset: Int, value: Int) {
            for (index in 0 until LENGTH_BYTES) {
                target[offset + index] = (value ushr (index * 8)).toByte()
            }
        }

        private fun readU32Le(source: ByteArray, offset: Int): Int {
            var value = 0L
            for (index in 0 until LENGTH_BYTES) {
                value = value or ((source[offset + index].toLong() and 0xff) shl (index * 8))
            }
            require(value <= Int.MAX_VALUE.toLong()) { "secure-element response length overflows Int" }
            return value.toInt()
        }
    }
}
