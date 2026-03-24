package org.hyperledger.iroha.sdk.core.model.instructions

import java.util.Optional
import org.hyperledger.iroha.sdk.client.IdentifierResolutionReceipt
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Encodes `ClaimIdentifier` instructions in wire-framed Norito format.
 *
 * The Torii identifier endpoints already expose the canonical signed receipt payload as
 * `signature_payload_hex`, so the encoder can reuse those bytes directly instead of trying to
 * re-encode receipt internals on the wallet.
 */
object ClaimIdentifierWirePayloadEncoder {

    const val WIRE_NAME = "identity::ClaimIdentifier"
    private const val SCHEMA_PATH = "iroha_data_model::isi::identifier::ClaimIdentifier"

    private val RAW_BYTE_VEC_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()
    private val OPTIONAL_SIGNATURE_ADAPTER: TypeAdapter<Optional<ByteArray>> = NoritoAdapters.option(RAW_BYTE_VEC_ADAPTER)
    private val OPTIONAL_PROOF_ADAPTER: TypeAdapter<Optional<ByteArray>> = NoritoAdapters.option(RAW_BYTE_VEC_ADAPTER)

    /** Encodes a signed `ClaimIdentifier` instruction as a wire-framed [InstructionBox]. */
    @JvmStatic
    fun encode(accountId: String, receipt: IdentifierResolutionReceipt): InstructionBox {
        val normalizedAccountId = requireNonBlank(accountId, "accountId")
        val receiptAccountId = requireNonBlank(receipt.accountId, "receipt.accountId")
        require(normalizedAccountId == receiptAccountId) { "ClaimIdentifier accountId must match receipt.accountId" }
        val accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId)
        val receiptPayload = decodeHex(receipt.signaturePayloadHex, "receipt.signaturePayloadHex")
        val signaturePayload = decodeHex(receipt.signature, "receipt.signature")
        val wirePayload = NoritoCodec.encode(
            ClaimIdentifierPayload(accountPayload, receiptPayload, signaturePayload),
            SCHEMA_PATH,
            ClaimIdentifierPayloadAdapter()
        )
        return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload)
    }

    private class ClaimIdentifierPayload(accountPayload: ByteArray, receiptPayload: ByteArray, signaturePayload: ByteArray) {
        val accountPayload: ByteArray = accountPayload.clone()
        val receiptPayload: ByteArray = receiptPayload.clone()
        val signaturePayload: ByteArray = signaturePayload.clone()
    }

    private class ClaimIdentifierPayloadAdapter : TypeAdapter<ClaimIdentifierPayload> {
        override fun encode(encoder: NoritoEncoder, value: ClaimIdentifierPayload) {
            encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.accountPayload)
            encodeSizedField(encoder, RECEIPT_ADAPTER, ReceiptPayload(value.receiptPayload, Optional.of(value.signaturePayload), Optional.empty()))
        }
        override fun decode(decoder: NoritoDecoder): ClaimIdentifierPayload = throw UnsupportedOperationException("Decoding ClaimIdentifier is not supported")
        companion object {
            private val PASSTHROUGH_ADAPTER = PassthroughBytesAdapter()
            private val RECEIPT_ADAPTER = ReceiptPayloadAdapter()
        }
    }

    private class ReceiptPayload(payloadBytes: ByteArray, val signatureBytes: Optional<ByteArray>, val proofBytes: Optional<ByteArray>) {
        val payloadBytes: ByteArray = payloadBytes.clone()
    }

    private class ReceiptPayloadAdapter : TypeAdapter<ReceiptPayload> {
        override fun encode(encoder: NoritoEncoder, value: ReceiptPayload) {
            encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.payloadBytes)
            encodeSizedField(encoder, OPTIONAL_SIGNATURE_ADAPTER, value.signatureBytes)
            encodeSizedField(encoder, OPTIONAL_PROOF_ADAPTER, value.proofBytes)
        }
        override fun decode(decoder: NoritoDecoder): ReceiptPayload = throw UnsupportedOperationException("Decoding identifier receipts is not supported")
        companion object { private val PASSTHROUGH_ADAPTER = PassthroughBytesAdapter() }
    }

    private class PassthroughBytesAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.isNotEmpty()) { "payload bytes must not be empty" }
            encoder.writeBytes(value)
        }
        override fun decode(decoder: NoritoDecoder): ByteArray = throw UnsupportedOperationException("Decoding passthrough payloads is not supported")
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun requireNonBlank(value: String?, field: String): String {
        val trimmed = value?.trim() ?: ""
        require(trimmed.isNotEmpty()) { "$field must not be blank" }
        return trimmed
    }

    private fun decodeHex(value: String, field: String): ByteArray {
        var trimmed = requireNonBlank(value, field)
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) trimmed = trimmed.substring(2)
        require(trimmed.length % 2 == 0) { "$field must contain an even number of hex characters" }
        val out = ByteArray(trimmed.length / 2)
        for (i in trimmed.indices step 2) {
            val high = Character.digit(trimmed[i], 16)
            val low = Character.digit(trimmed[i + 1], 16)
            require(high >= 0 && low >= 0) { "$field contains non-hex characters" }
            out[i / 2] = ((high shl 4) or low).toByte()
        }
        return out
    }
}
