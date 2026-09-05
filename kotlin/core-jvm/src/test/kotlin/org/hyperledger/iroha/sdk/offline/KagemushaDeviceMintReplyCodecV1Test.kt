// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.offline

import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.test.Test
import kotlin.test.assertFailsWith
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

class KagemushaDeviceMintReplyCodecV1Test {
    @Test
    fun `operations 14 and 15 accept the complete mint construction bundle`() {
        val authorization = byteArrayOf(0x11, 0x12, 0x13)
        val encryptedCredit = byteArrayOf(0x21, 0x22)

        for (operation in listOf(14, 15)) {
            val archive = mintReply(operation, authorization, encryptedCredit)
            KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(
                operation,
                archive,
            )
        }
    }

    @Test
    fun `operations 14 and 15 reject the obsolete authorization-only reply`() {
        val authorization = byteArrayOf(0x11, 0x12, 0x13)

        for (operation in listOf(14, 15)) {
            val archive = framedReply(
                operation,
                field(byteVector(authorization)),
            )
            assertFailsWith<IllegalArgumentException> {
                KagemushaDeviceOperationCodecV1.decodeControlReplyAfterAuthentication(
                    operation,
                    archive,
                )
            }
        }
    }

    private fun mintReply(
        operation: Int,
        authorization: ByteArray,
        encryptedCredit: ByteArray,
    ): ByteArray = framedReply(
        operation,
        field(byteVector(authorization)) + field(byteVector(encryptedCredit)),
    )

    private fun framedReply(operation: Int, body: ByteArray): ByteArray {
        val payload = field(u16(1)) + field(byteArrayOf(operation.toByte())) + body
        val archive = NoritoCodec.encode(payload, MINT_REPLY_SCHEMA, RAW_BYTES)
        val padding = (8 - NoritoHeader.HEADER_LENGTH % 8) % 8
        if (padding == 0) return archive
        return ByteArray(archive.size + padding).also { result ->
            archive.copyInto(result, endIndex = NoritoHeader.HEADER_LENGTH)
            archive.copyInto(
                result,
                destinationOffset = NoritoHeader.HEADER_LENGTH + padding,
                startIndex = NoritoHeader.HEADER_LENGTH,
            )
        }
    }

    private fun field(value: ByteArray): ByteArray = compactLength(value.size) + value

    private fun byteVector(value: ByteArray): ByteArray =
        ByteBuffer.allocate(Long.SIZE_BYTES + value.size)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putLong(value.size.toLong())
            .put(value)
            .array()

    private fun u16(value: Int): ByteArray = ByteBuffer.allocate(Short.SIZE_BYTES)
        .order(ByteOrder.LITTLE_ENDIAN)
        .putShort(value.toShort())
        .array()

    private fun compactLength(value: Int): ByteArray {
        require(value >= 0)
        var remaining = value
        val encoded = ArrayList<Byte>()
        do {
            val current = remaining and 0x7f
            remaining = remaining ushr 7
            encoded += (current or if (remaining == 0) 0 else 0x80).toByte()
        } while (remaining != 0)
        return ByteArray(encoded.size) { encoded[it] }
    }

    private companion object {
        const val MINT_REPLY_SCHEMA =
            "iroha.kagemusha.device.v1.mint-construction-bundle-reply"

        val RAW_BYTES = object : TypeAdapter<ByteArray> {
            override fun encode(encoder: NoritoEncoder, value: ByteArray) =
                encoder.writeBytes(value)

            override fun decode(decoder: NoritoDecoder): ByteArray =
                decoder.readBytes(decoder.remaining())
        }
    }
}
