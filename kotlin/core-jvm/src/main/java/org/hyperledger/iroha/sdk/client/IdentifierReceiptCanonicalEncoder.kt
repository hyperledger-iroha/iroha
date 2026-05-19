package org.hyperledger.iroha.sdk.client

import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Canonical Norito encoder for identifier receipt payloads. */
object IdentifierReceiptCanonicalEncoder {
    private val STRING_ADAPTER: TypeAdapter<String> = NoritoAdapters.stringAdapter()
    private val U64_ADAPTER: TypeAdapter<Long> = NoritoAdapters.uint(64)
    private val SIGNATURE_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.byteVecAdapter()
    private val RAW_BYTE_VEC_ADAPTER: TypeAdapter<ByteArray> = NoritoAdapters.rawByteVecAdapter()

    @JvmStatic
    fun encodePayload(payload: IdentifierResolutionPayload): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, PassthroughBytesAdapter, encodePolicyId(payload.policyId))
        encodeSizedField(writer, PassthroughBytesAdapter, encodeExecution(payload.execution))
        encodeSizedField(writer, PassthroughBytesAdapter, encodeOutputOpening(payload.opening))
        encodeSizedField(writer, PassthroughBytesAdapter, encodeOpaqueHash(payload.opaqueId, "opaque:", "payload.opaque_id"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.receiptHash, "payload.receipt_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, encodeOpaqueHash(payload.uaid, "uaid:", "payload.uaid"))
        encodeSizedField(writer, PassthroughBytesAdapter, TransferWirePayloadEncoder.encodeAccountIdPayload(payload.accountId))
        return writer.toByteArray()
    }

    @JvmStatic
    fun encodeAttestation(attestation: IdentifierReceiptAttestation): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        when (attestation.kind.lowercase()) {
            "signed" -> {
                writer.writeUInt(0, 32)
                encodeSizedField(writer, SIGNATURE_ADAPTER, decodeHex(requireNotNull(attestation.signature) {
                    "signed attestation requires signature"
                }, "attestation.signature"))
            }
            "proof" -> {
                writer.writeUInt(1, 32)
                encodeSizedField(
                    writer,
                    ProofBoxAdapter,
                    ProofBoxPayload(
                        requireNotNull(attestation.proofBackend) { "proof attestation requires proofBackend" },
                        java.util.Base64.getDecoder().decode(
                            requireNotNull(attestation.proofB64) { "proof attestation requires proofB64" }
                        )
                    )
                )
            }
            else -> error("attestation.kind must be signed or proof")
        }
        return writer.toByteArray()
    }

    private fun encodePolicyId(raw: String): ByteArray {
        val parts = raw.trim().split("#", limit = 2)
        require(parts.size == 2 && parts[0].isNotBlank() && parts[1].isNotBlank()) {
            "payload.policy_id must use kind#rule"
        }
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, STRING_ADAPTER, parts[0].trim())
        encodeSizedField(writer, STRING_ADAPTER, parts[1].trim())
        return writer.toByteArray()
    }

    private fun encodeExecution(execution: IdentifierResolutionExecutionPayload): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, PassthroughBytesAdapter, encodeProgramId(execution.programId))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.programDigest, "payload.execution.program_digest"))
        encodeSizedField(writer, U32Adapter, backendTag(execution.backend).toLong())
        encodeSizedField(writer, U32Adapter, verificationModeTag(execution.verificationMode).toLong())
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.inputCiphertextHash, "payload.execution.input_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.outputCiphertextHash, "payload.execution.output_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.parameterDigest, "payload.execution.parameter_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.evaluationKeyDigest, "payload.execution.evaluation_key_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.outputHash, "payload.execution.output_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.associatedDataHash, "payload.execution.associated_data_hash"))
        encodeSizedField(writer, U64_ADAPTER, execution.executedAtMs)
        encodeSizedField(writer, OptionalU64Adapter, execution.expiresAtMs)
        return writer.toByteArray()
    }

    private fun encodeOutputOpening(opening: RamLfeOutputOpening): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, PassthroughBytesAdapter, encodeOutputOpeningPayload(opening.payload))
        encodeSizedField(writer, SIGNATURE_ADAPTER, decodeHex(opening.signature, "payload.opening.signature"))
        return writer.toByteArray()
    }

    private fun encodeOutputOpeningPayload(payload: RamLfeOutputOpeningPayload): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, PassthroughBytesAdapter, encodeProgramId(payload.programId))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.inputCiphertextHash, "payload.opening.payload.input_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.outputCiphertextHash, "payload.opening.payload.output_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.parameterDigest, "payload.opening.payload.parameter_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.evaluationKeyDigest, "payload.opening.payload.evaluation_key_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.openedOutputHash, "payload.opening.payload.opened_output_hash"))
        encodeSizedField(writer, U64_ADAPTER, payload.openedAtMs)
        encodeSizedField(writer, OptionalU64Adapter, payload.expiresAtMs)
        return writer.toByteArray()
    }

    private fun encodeProgramId(raw: String): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, STRING_ADAPTER, requireNonBlank(raw, "payload.execution.program_id"))
        return writer.toByteArray()
    }

    private fun backendTag(raw: String): Int = when (raw.trim().lowercase()) {
        "hkdf-sha3-512-prf-v1" -> 0
        "bfv-affine-sha3-256-v1" -> 1
        "bfv-programmed-sha3-256-v1" -> 2
        else -> error("unsupported RAM-LFE backend: $raw")
    }

    private fun verificationModeTag(raw: String): Int = when (raw.trim().lowercase()) {
        "signed" -> 0
        "proof" -> 1
        else -> error("unsupported RAM-LFE verification mode: $raw")
    }

    private fun encodePrefixedHash(raw: String, prefix: String, field: String): ByteArray {
        val normalized = raw.trim().lowercase()
        val body = if (normalized.startsWith(prefix)) normalized.substring(prefix.length) else normalized
        return decodeHash(body, field)
    }

    private fun encodeOpaqueHash(raw: String, prefix: String, field: String): ByteArray {
        val hash = encodePrefixedHash(raw, prefix, field)
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        val compact = (writer.flags and NoritoHeader.COMPACT_LEN) != 0
        writer.writeLength(hash.size.toLong(), compact)
        writer.writeBytes(hash)
        return writer.toByteArray()
    }

    private fun decodeHash(raw: String, field: String): ByteArray {
        var body = requireNonBlank(raw, field)
        if (body.lowercase().startsWith("hash:")) {
            body = body.substring("hash:".length)
        }
        val suffixIndex = body.indexOf('#')
        if (suffixIndex >= 0) {
            body = body.substring(0, suffixIndex)
        }
        val bytes = decodeHex(body, field)
        require(bytes.size == 32) { "$field must contain 32 bytes" }
        return bytes
    }

    private fun decodeHex(raw: String, field: String): ByteArray {
        var trimmed = requireNonBlank(raw, field)
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

    private fun requireNonBlank(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must not be blank" }
        return trimmed
    }

    private fun <T> encodeSizedField(encoder: NoritoEncoder, adapter: TypeAdapter<T>, value: T) {
        val child = encoder.childEncoder()
        adapter.encode(child, value)
        val payload = child.toByteArray()
        val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
        encoder.writeLength(payload.size.toLong(), compact)
        encoder.writeBytes(payload)
    }

    private object U32Adapter : TypeAdapter<Long> {
        override fun encode(encoder: NoritoEncoder, value: Long) {
            encoder.writeUInt(value, 32)
        }
        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): Long =
            throw UnsupportedOperationException("decode not supported")
    }

    private object OptionalU64Adapter : TypeAdapter<Long?> {
        override fun encode(encoder: NoritoEncoder, value: Long?) {
            if (value == null) {
                encoder.writeByte(0)
            } else {
                encoder.writeByte(1)
                encodeSizedField(encoder, U64_ADAPTER, value)
            }
        }
        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): Long? =
            throw UnsupportedOperationException("decode not supported")
    }

    private object PassthroughBytesAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
        }
        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): ByteArray =
            throw UnsupportedOperationException("decode not supported")
    }

    private data class ProofBoxPayload(val backend: String, val bytes: ByteArray)

    private object ProofBoxAdapter : TypeAdapter<ProofBoxPayload> {
        override fun encode(encoder: NoritoEncoder, value: ProofBoxPayload) {
            encodeSizedField(encoder, STRING_ADAPTER, value.backend)
            encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value.bytes)
        }
        override fun decode(decoder: org.hyperledger.iroha.sdk.norito.NoritoDecoder): ProofBoxPayload =
            throw UnsupportedOperationException("decode not supported")
    }
}
