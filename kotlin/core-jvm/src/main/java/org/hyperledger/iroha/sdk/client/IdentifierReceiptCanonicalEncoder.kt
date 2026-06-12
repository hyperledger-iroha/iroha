package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.instructions.TransferWirePayloadEncoder
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
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
        encodeSizedField(
            writer,
            PassthroughBytesAdapter,
            TransferWirePayloadEncoder.encodeAccountIdPayload(
                requireCanonicalI105Address(payload.accountId, "payload.account_id"),
            ),
        )
        return writer.toByteArray()
    }

    @JvmStatic
    internal fun decodePayload(encoded: ByteArray): IdentifierResolutionPayload {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val policyId = decodePolicyId(decodeSizedField(decoder, PassthroughBytesAdapter, "payload.policy_id"))
        val execution = decodeExecution(decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution"))
        val opening = decodeOutputOpening(decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening"))
        val opaqueId = decodeOpaqueHash(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opaque_id"),
            "opaque:",
            "payload.opaque_id",
        )
        val receiptHash = hashHex(decodeSizedField(decoder, PassthroughBytesAdapter, "payload.receipt_hash"))
        val uaid = decodeOpaqueHash(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.uaid"),
            "uaid:",
            "payload.uaid",
        )
        val accountId = TransferWirePayloadEncoder.decodeAccountIdPayload(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.account_id"),
            decoder.flags,
            decoder.flagsHint,
        )
        require(decoder.remaining() == 0) { "Trailing bytes after identifier receipt payload" }
        return IdentifierResolutionPayload(policyId, execution, opening, opaqueId, receiptHash, uaid, accountId)
    }

    @JvmStatic
    fun encodeAttestation(attestation: IdentifierReceiptAttestation): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        when (requireExactNonBlankString(attestation.kind, "attestation.kind")) {
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
                        requireExactNonBlankString(
                            requireNotNull(attestation.proofBackend) {
                                "proof attestation requires proofBackend"
                            },
                            "attestation.proof_backend",
                        ),
                        java.util.Base64.getDecoder().decode(
                            requireExactNonBlankString(
                                requireNotNull(attestation.proofB64) { "proof attestation requires proofB64" },
                                "attestation.proof_b64",
                            )
                        )
                    )
                )
            }
            else -> throw IllegalArgumentException("attestation.kind must be signed or proof")
        }
        return writer.toByteArray()
    }

    @JvmStatic
    internal fun decodeAttestation(encoded: ByteArray): IdentifierReceiptAttestation {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val tag = decoder.readUInt(32)
        val attestation = when (tag) {
            0L -> {
                val signature = decodeSizedField(decoder, SIGNATURE_ADAPTER, "attestation.signature")
                IdentifierReceiptAttestation("signed", hexLower(signature), null, null)
            }
            1L -> {
                val proof = decodeSizedField(decoder, ProofBoxAdapter, "attestation.proof")
                IdentifierReceiptAttestation(
                    "proof",
                    null,
                    requireExactNonBlankString(proof.backend, "attestation.proof_backend"),
                    Base64.getEncoder().encodeToString(proof.bytes),
                )
            }
            else -> throw IllegalArgumentException("Unsupported identifier attestation tag: $tag")
        }
        require(decoder.remaining() == 0) { "Trailing bytes after identifier receipt attestation" }
        return attestation
    }

    private fun encodePolicyId(raw: String): ByteArray {
        val value = requireExactNonBlankString(raw, "payload.policy_id")
        val parts = value.split("#", limit = 2)
        require(parts.size == 2 && parts[0].isNotEmpty() && parts[1].isNotEmpty()) {
            "payload.policy_id must use kind#rule"
        }
        val kind = requireExactNonBlankString(parts[0], "payload.policy_id.kind")
        val rule = requireExactNonBlankString(parts[1], "payload.policy_id.rule")
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, STRING_ADAPTER, kind)
        encodeSizedField(writer, STRING_ADAPTER, rule)
        return writer.toByteArray()
    }

    private fun decodePolicyId(encoded: ByteArray): String {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val kind = decodeSizedField(decoder, STRING_ADAPTER, "payload.policy_id.kind")
        val rule = decodeSizedField(decoder, STRING_ADAPTER, "payload.policy_id.rule")
        require(decoder.remaining() == 0) { "Trailing bytes after payload.policy_id" }
        return "$kind#$rule"
    }

    private fun encodeExecution(execution: IdentifierResolutionExecutionPayload): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(
            writer,
            PassthroughBytesAdapter,
            encodeProgramId(execution.programId, "payload.execution.program_id"),
        )
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.programDigest, "payload.execution.program_digest"))
        encodeSizedField(writer, U32Adapter, backendTag(execution.backend).toLong())
        encodeSizedField(writer, U32Adapter, verificationModeTag(execution.verificationMode).toLong())
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.inputCiphertextHash, "payload.execution.input_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.outputCiphertextHash, "payload.execution.output_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.parameterDigest, "payload.execution.parameter_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.evaluationKeyDigest, "payload.execution.evaluation_key_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.outputHash, "payload.execution.output_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(execution.associatedDataHash, "payload.execution.associated_data_hash"))
        encodeSizedField(writer, U64_ADAPTER, requireU64(execution.executedAtMs, "payload.execution.executed_at_ms"))
        encodeSizedField(
            writer,
            OptionalU64Adapter,
            execution.expiresAtMs?.let { requireU64(it, "payload.execution.expires_at_ms") },
        )
        return writer.toByteArray()
    }

    private fun decodeExecution(encoded: ByteArray): IdentifierResolutionExecutionPayload {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val programId = decodeProgramId(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.program_id")
        )
        val programDigest = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.program_digest")
        )
        val backend = backendName(
            Math.toIntExact(decodeSizedField(decoder, U32Adapter, "payload.execution.backend"))
        )
        val verificationMode = verificationModeName(
            Math.toIntExact(decodeSizedField(decoder, U32Adapter, "payload.execution.verification_mode"))
        )
        val inputCiphertextHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.input_ciphertext_hash")
        )
        val outputCiphertextHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.output_ciphertext_hash")
        )
        val parameterDigest = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.parameter_digest")
        )
        val evaluationKeyDigest = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.evaluation_key_digest")
        )
        val outputHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.output_hash")
        )
        val associatedDataHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.execution.associated_data_hash")
        )
        val executedAtMs = decodeSizedField(decoder, U64_ADAPTER, "payload.execution.executed_at_ms")
        val expiresAtMs = decodeSizedField(decoder, OptionalU64Adapter, "payload.execution.expires_at_ms")
        require(decoder.remaining() == 0) { "Trailing bytes after payload.execution" }
        return IdentifierResolutionExecutionPayload(
            programId,
            programDigest,
            backend,
            verificationMode,
            inputCiphertextHash,
            outputCiphertextHash,
            parameterDigest,
            evaluationKeyDigest,
            outputHash,
            associatedDataHash,
            executedAtMs,
            expiresAtMs,
        )
    }

    private fun encodeOutputOpening(opening: RamLfeOutputOpening): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, PassthroughBytesAdapter, encodeOutputOpeningPayload(opening.payload))
        encodeSizedField(writer, SIGNATURE_ADAPTER, decodeHex(opening.signature, "payload.opening.signature"))
        return writer.toByteArray()
    }

    private fun decodeOutputOpening(encoded: ByteArray): RamLfeOutputOpening {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val payload = decodeOutputOpeningPayload(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload")
        )
        val signature = hexLower(decodeSizedField(decoder, SIGNATURE_ADAPTER, "payload.opening.signature"))
        require(decoder.remaining() == 0) { "Trailing bytes after payload.opening" }
        return RamLfeOutputOpening(payload, signature)
    }

    private fun encodeOutputOpeningPayload(payload: RamLfeOutputOpeningPayload): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(
            writer,
            PassthroughBytesAdapter,
            encodeProgramId(payload.programId, "payload.opening.payload.program_id"),
        )
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.inputCiphertextHash, "payload.opening.payload.input_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.outputCiphertextHash, "payload.opening.payload.output_ciphertext_hash"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.parameterDigest, "payload.opening.payload.parameter_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.evaluationKeyDigest, "payload.opening.payload.evaluation_key_digest"))
        encodeSizedField(writer, PassthroughBytesAdapter, decodeHash(payload.openedOutputHash, "payload.opening.payload.opened_output_hash"))
        encodeSizedField(writer, U64_ADAPTER, requireU64(payload.openedAtMs, "payload.opening.payload.opened_at_ms"))
        encodeSizedField(
            writer,
            OptionalU64Adapter,
            payload.expiresAtMs?.let { requireU64(it, "payload.opening.payload.expires_at_ms") },
        )
        return writer.toByteArray()
    }

    private fun decodeOutputOpeningPayload(encoded: ByteArray): RamLfeOutputOpeningPayload {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val programId = decodeProgramId(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.program_id")
        )
        val inputCiphertextHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.input_ciphertext_hash")
        )
        val outputCiphertextHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.output_ciphertext_hash")
        )
        val parameterDigest = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.parameter_digest")
        )
        val evaluationKeyDigest = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.evaluation_key_digest")
        )
        val openedOutputHash = hashHex(
            decodeSizedField(decoder, PassthroughBytesAdapter, "payload.opening.payload.opened_output_hash")
        )
        val openedAtMs = decodeSizedField(decoder, U64_ADAPTER, "payload.opening.payload.opened_at_ms")
        val expiresAtMs = decodeSizedField(decoder, OptionalU64Adapter, "payload.opening.payload.expires_at_ms")
        require(decoder.remaining() == 0) { "Trailing bytes after payload.opening.payload" }
        return RamLfeOutputOpeningPayload(
            programId,
            inputCiphertextHash,
            outputCiphertextHash,
            parameterDigest,
            evaluationKeyDigest,
            openedOutputHash,
            openedAtMs,
            expiresAtMs,
        )
    }

    private fun encodeProgramId(raw: String, field: String): ByteArray {
        val writer = NoritoEncoder(NoritoCodec.DEFAULT_FLAGS)
        encodeSizedField(writer, STRING_ADAPTER, requireExactNonBlankString(raw, field))
        return writer.toByteArray()
    }

    private fun decodeProgramId(encoded: ByteArray): String {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val programId = decodeSizedField(decoder, STRING_ADAPTER, "program_id")
        require(decoder.remaining() == 0) { "Trailing bytes after program_id" }
        return programId
    }

    private fun backendTag(raw: String): Int = when (
        requireExactNonBlankString(raw, "payload.execution.backend")
    ) {
        "hkdf-sha3-512-prf-v1" -> 0
        "bfv-affine-sha3-256-v1" -> 1
        "bfv-programmed-sha3-256-v1" -> 2
        else -> throw IllegalArgumentException("unsupported RAM-LFE backend: $raw")
    }

    private fun verificationModeTag(raw: String): Int = when (
        requireExactNonBlankString(raw, "payload.execution.verification_mode")
    ) {
        "signed" -> 0
        "proof" -> 1
        else -> throw IllegalArgumentException("unsupported RAM-LFE verification mode: $raw")
    }

    private fun backendName(tag: Int): String = when (tag) {
        0 -> "hkdf-sha3-512-prf-v1"
        1 -> "bfv-affine-sha3-256-v1"
        2 -> "bfv-programmed-sha3-256-v1"
        else -> throw IllegalArgumentException("unsupported RAM-LFE backend tag: $tag")
    }

    private fun verificationModeName(tag: Int): String = when (tag) {
        0 -> "signed"
        1 -> "proof"
        else -> throw IllegalArgumentException("unsupported RAM-LFE verification mode tag: $tag")
    }

    private fun encodePrefixedHash(raw: String, prefix: String, field: String): ByteArray {
        val normalized = requireExactNonBlankString(raw, field).lowercase()
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

    private fun decodeOpaqueHash(encoded: ByteArray, prefix: String, field: String): String {
        val decoder = NoritoDecoder(encoded, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION)
        val length = decoder.readLength((decoder.flags and NoritoHeader.COMPACT_LEN) != 0)
        require(length == 32L) { "$field must contain 32 bytes" }
        val hash = decoder.readBytes(32)
        require(decoder.remaining() == 0) { "Trailing bytes after $field" }
        return prefix + hashHex(hash)
    }

    private fun decodeHash(raw: String, field: String): ByteArray {
        var body = requireExactNonBlankString(raw, field)
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
        require(trimmed == raw) { "$field must not contain surrounding whitespace" }
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

    private fun requireExactNonBlankString(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must not be blank" }
        require(trimmed == value) { "$field must not contain surrounding whitespace" }
        return value
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

    private fun hashHex(bytes: ByteArray): String {
        require(bytes.size == 32) { "hash value must contain 32 bytes" }
        return hexLower(bytes)
    }

    private fun hexLower(bytes: ByteArray): String = buildString(bytes.size * 2) {
        for (byte in bytes) {
            append("%02x".format(byte.toInt() and 0xFF))
        }
    }

    private fun requireU64(value: Long, field: String): Long {
        require(value >= 0L) { "$field must be a non-negative u64" }
        return value
    }

    private object U32Adapter : TypeAdapter<Long> {
        override fun encode(encoder: NoritoEncoder, value: Long) {
            encoder.writeUInt(value, 32)
        }
        override fun decode(decoder: NoritoDecoder): Long = decoder.readUInt(32)
    }

    private object OptionalU64Adapter : TypeAdapter<Long?> {
        override fun encode(encoder: NoritoEncoder, value: Long?) {
            if (value == null) {
                encoder.writeByte(0)
            } else {
                encoder.writeByte(1)
                encodeSizedField(encoder, U64_ADAPTER, requireU64(value, "optional u64"))
            }
        }
        override fun decode(decoder: NoritoDecoder): Long? {
            val tag = decoder.readByte()
            if (tag == 0) return null
            require(tag == 1) { "Invalid optional u64 tag: $tag" }
            return decodeSizedField(decoder, U64_ADAPTER, "optional u64")
        }
    }

    private object PassthroughBytesAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeBytes(value)
        }
        override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(decoder.remaining())
    }

    private data class ProofBoxPayload(val backend: String, val bytes: ByteArray)

    private object ProofBoxAdapter : TypeAdapter<ProofBoxPayload> {
        override fun encode(encoder: NoritoEncoder, value: ProofBoxPayload) {
            encodeSizedField(encoder, STRING_ADAPTER, value.backend)
            encodeSizedField(encoder, RAW_BYTE_VEC_ADAPTER, value.bytes)
        }
        override fun decode(decoder: NoritoDecoder): ProofBoxPayload {
            val backend = decodeSizedField(decoder, STRING_ADAPTER, "proof.backend")
            val bytes = decodeSizedField(decoder, RAW_BYTE_VEC_ADAPTER, "proof.bytes")
            return ProofBoxPayload(backend, bytes)
        }
    }
}
