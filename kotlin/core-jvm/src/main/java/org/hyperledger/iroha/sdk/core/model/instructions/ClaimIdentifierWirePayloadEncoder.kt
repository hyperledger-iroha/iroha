package org.hyperledger.iroha.sdk.core.model.instructions

import org.hyperledger.iroha.sdk.client.IdentifierResolutionReceipt
import org.hyperledger.iroha.sdk.client.IdentifierReceiptCanonicalEncoder
import org.hyperledger.iroha.sdk.core.model.InstructionBox
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/**
 * Encodes `ClaimIdentifier` instructions in wire-framed Norito format.
 *
 * The Torii identifier endpoints expose canonical `{ payload, attestation }` receipts, so the
 * encoder derives payload bytes from the structured payload and encodes the explicit attestation.
 */
object ClaimIdentifierWirePayloadEncoder {

    const val WIRE_NAME = "identity::ClaimIdentifier"
    private const val SCHEMA_PATH = "iroha_data_model::isi::identifier::ClaimIdentifier"

    /** Encodes a `ClaimIdentifier` instruction as a wire-framed [InstructionBox]. */
    @JvmStatic
    fun encode(accountId: String, receipt: IdentifierResolutionReceipt): InstructionBox {
        val normalizedAccountId = requireNonBlank(accountId, "accountId")
        val receiptAccountId = requireNonBlank(receipt.accountId, "receipt.accountId")
        require(normalizedAccountId == receiptAccountId) { "ClaimIdentifier accountId must match receipt.accountId" }
        val accountPayload = TransferWirePayloadEncoder.encodeAccountIdPayload(normalizedAccountId)
        val receiptPayload = IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload)
        val attestationPayload = IdentifierReceiptCanonicalEncoder.encodeAttestation(receipt.attestation)
        val wirePayload = NoritoCodec.encode(
            ClaimIdentifierPayload(accountPayload, receiptPayload, attestationPayload),
            SCHEMA_PATH,
            ClaimIdentifierPayloadAdapter()
        )
        return InstructionBox.fromWirePayload(WIRE_NAME, wirePayload)
    }

    private class ClaimIdentifierPayload(accountPayload: ByteArray, receiptPayload: ByteArray, attestationPayload: ByteArray) {
        val accountPayload: ByteArray = accountPayload.clone()
        val receiptPayload: ByteArray = receiptPayload.clone()
        val attestationPayload: ByteArray = attestationPayload.clone()
    }

    private class ClaimIdentifierPayloadAdapter : TypeAdapter<ClaimIdentifierPayload> {
        override fun encode(encoder: NoritoEncoder, value: ClaimIdentifierPayload) {
            encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.accountPayload)
            encodeSizedField(encoder, RECEIPT_ADAPTER, ReceiptPayload(value.receiptPayload, value.attestationPayload))
        }
        override fun decode(decoder: NoritoDecoder): ClaimIdentifierPayload = throw UnsupportedOperationException("Decoding ClaimIdentifier is not supported")
        companion object {
            private val PASSTHROUGH_ADAPTER = PassthroughBytesAdapter()
            private val RECEIPT_ADAPTER = ReceiptPayloadAdapter()
        }
    }

    private class ReceiptPayload(payloadBytes: ByteArray, attestationBytes: ByteArray) {
        val payloadBytes: ByteArray = payloadBytes.clone()
        val attestationBytes: ByteArray = attestationBytes.clone()
    }

    private class ReceiptPayloadAdapter : TypeAdapter<ReceiptPayload> {
        override fun encode(encoder: NoritoEncoder, value: ReceiptPayload) {
            encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.payloadBytes)
            encodeSizedField(encoder, PASSTHROUGH_ADAPTER, value.attestationBytes)
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
}
