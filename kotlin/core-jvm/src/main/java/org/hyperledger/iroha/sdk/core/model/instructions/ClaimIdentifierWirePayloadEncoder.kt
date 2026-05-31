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

    /** Decodes a Norito-framed `ClaimIdentifier` payload. */
    @JvmStatic
    internal fun decodePayload(wirePayload: ByteArray): DecodedClaimIdentifierPayload {
        val payload = NoritoCodec.decode(wirePayload, ClaimIdentifierPayloadAdapter(), SCHEMA_PATH)
        return DecodedClaimIdentifierPayload(
            accountId = TransferWirePayloadEncoder.decodeAccountIdPayload(payload.accountPayload),
            receiptPayloadBytes = payload.receiptPayload,
            attestationPayloadBytes = payload.attestationPayload,
        )
    }

    internal data class DecodedClaimIdentifierPayload(
        val accountId: String,
        val receiptPayloadBytes: ByteArray,
        val attestationPayloadBytes: ByteArray,
    ) {
        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is DecodedClaimIdentifierPayload) return false
            return accountId == other.accountId &&
                receiptPayloadBytes.contentEquals(other.receiptPayloadBytes) &&
                attestationPayloadBytes.contentEquals(other.attestationPayloadBytes)
        }

        override fun hashCode(): Int {
            var result = accountId.hashCode()
            result = 31 * result + receiptPayloadBytes.contentHashCode()
            result = 31 * result + attestationPayloadBytes.contentHashCode()
            return result
        }
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
        override fun decode(decoder: NoritoDecoder): ClaimIdentifierPayload {
            val accountPayload = decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "ClaimIdentifier.account_id")
            val receipt = decodeSizedField(decoder, RECEIPT_ADAPTER, "ClaimIdentifier.receipt")
            return ClaimIdentifierPayload(accountPayload, receipt.payloadBytes, receipt.attestationBytes)
        }
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
        override fun decode(decoder: NoritoDecoder): ReceiptPayload {
            val payloadBytes = decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "IdentifierReceipt.payload")
            val attestationBytes = decodeSizedField(decoder, PASSTHROUGH_ADAPTER, "IdentifierReceipt.attestation")
            return ReceiptPayload(payloadBytes, attestationBytes)
        }
        companion object { private val PASSTHROUGH_ADAPTER = PassthroughBytesAdapter() }
    }

    private class PassthroughBytesAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.isNotEmpty()) { "payload bytes must not be empty" }
            encoder.writeBytes(value)
        }
        override fun decode(decoder: NoritoDecoder): ByteArray {
            val payload = decoder.readBytes(decoder.remaining())
            require(payload.isNotEmpty()) { "payload bytes must not be empty" }
            return payload
        }
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private fun <T> decodeSizedField(decoder: NoritoDecoder, adapter: TypeAdapter<T>, fieldName: String): T {
        val length = decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0)
        require(length <= Int.MAX_VALUE) { "$fieldName payload too large" }
        val payload = decoder.readBytes(length.toInt())
        val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
        val value = adapter.decode(child)
        require(child.remaining() == 0) { "Trailing bytes after $fieldName payload" }
        return value
    }

    private fun requireNonBlank(value: String?, field: String): String {
        val trimmed = value?.trim() ?: ""
        require(trimmed.isNotEmpty()) { "$field must not be blank" }
        return trimmed
    }
}
