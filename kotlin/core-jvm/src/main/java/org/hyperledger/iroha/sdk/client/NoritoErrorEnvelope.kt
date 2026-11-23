// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.norito.Varint

/** Diagnostic fields from the Torii-owned canonical ErrorEnvelope. */
internal class NoritoErrorEnvelope private constructor(
    val code: String,
    val message: String,
    val hasDetails: Boolean,
    val rejectCode: String?,
) {
    companion object {
        private val schemaHash = SchemaHash.hash16("iroha_torii_shared::ErrorEnvelope")
        // Rust ErrorDetails declares ivm_fault, layer, reject_code, then its remaining members.
        private const val REJECT_CODE_FIELD = 2
        private const val DETAIL_FIELD_COUNT = 26

        fun decode(body: ByteArray): NoritoErrorEnvelope {
            val (header, payload) = NoritoHeader.decodeView(ByteBuffer.wrap(body), schemaHash)
            require(header.compression == NoritoHeader.COMPRESSION_NONE)
            require(header.flags == NoritoHeader.COMPACT_LEN)
            require(body.size - NoritoHeader.HEADER_LENGTH == header.payloadLength)
            header.validateChecksum(payload.duplicate())

            val code = payload.readField().readString()
            val message = payload.readField().readString()
            val details = payload.readField().readOption()
            require(!payload.hasRemaining()) { "trailing error-envelope payload" }

            var rejectCode: String? = null
            if (details != null) {
                repeat(DETAIL_FIELD_COUNT) { index ->
                    val value = details.readField().readOption()
                    if (index == REJECT_CODE_FIELD) rejectCode = value?.readString()
                }
                require(!details.hasRemaining()) { "trailing error details" }
            }
            return NoritoErrorEnvelope(code, message, details != null, rejectCode)
        }

        private fun ByteBuffer.readField(): ByteBuffer {
            val length = Varint.decode(this).value
            require(length in 0..remaining().toLong()) { "invalid error-envelope field length" }
            val field = slice()
            field.limit(length.toInt())
            position(position() + length.toInt())
            return field
        }

        private fun ByteBuffer.readOption(): ByteBuffer? {
            require(hasRemaining()) { "missing error-envelope option" }
            val value = when (get().toInt()) {
                0 -> null
                1 -> readField()
                else -> throw IllegalArgumentException("invalid error-envelope option")
            }
            require(!hasRemaining()) { "trailing error-envelope option" }
            return value
        }

        private fun ByteBuffer.readString(): String {
            val bytes = readField()
            require(!hasRemaining()) { "trailing error-envelope string" }
            try {
                return StandardCharsets.UTF_8.newDecoder().decode(bytes).toString()
            } catch (cause: CharacterCodingException) {
                throw IllegalArgumentException("invalid error-envelope UTF-8", cause)
            }
        }
    }
}
