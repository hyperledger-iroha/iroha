package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.sccp.SccpReplayV1
import org.hyperledger.iroha.sdk.sccp.SccpSparseMerkleWitnessV1
import org.hyperledger.iroha.sdk.sccp.SccpV1

/** Exact request payload for POST /v1/bridge/proofs/submit. */
class SccpDestinationProofSubmitRequest(
    authority: String,
    destinationProofB64: String,
    feePayment: FeePaymentIntent,
) {
    val authority: String = requireCanonicalSccpAuthority(authority)
    val feePayment: FeePaymentIntent = feePayment
    val destinationProofB64: String = destinationProofB64.also {
        validateCanonicalSccpNoritoBase64(
            it,
            "destinationProofB64",
            SCCP_MAX_DESTINATION_ARTIFACT_BYTES,
            SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME,
        )
    }
    /** Exact JSON object accepted by Torii; route overrides are unrepresentable. */
    fun toJsonMap(): Map<String, Any> = linkedMapOf<String, Any>(
        "authority" to authority,
        "fee_payment" to feePayment.toJsonMap(),
        "destination_proof_b64" to destinationProofB64,
    )

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

/** Exact native-proof request payload for POST /v1/bridge/messages. */
class SccpNativeMessageSubmitRequest(
    authority: String,
    nativeProofB64: String,
    replayWitnessB64: String,
    feePayment: FeePaymentIntent,
) {
    val authority: String = requireCanonicalSccpAuthority(authority)
    val feePayment: FeePaymentIntent = feePayment
    val nativeProofB64: String = nativeProofB64.also {
        validateCanonicalSccpNoritoBase64(
            it,
            "nativeProofB64",
            SCCP_MAX_NATIVE_PROOF_BYTES,
            SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME,
        )
    }
    val replayWitnessB64: String = replayWitnessB64.also {
        validateCanonicalSccpReplayWitnessBase64(
            it,
            "replayWitnessB64",
        )
    }
    /** Exact JSON object accepted by Torii; settlement selectors are unrepresentable. */
    fun toJsonMap(): Map<String, Any> = linkedMapOf<String, Any>(
        "authority" to authority,
        "fee_payment" to feePayment.toJsonMap(),
        "native_proof_b64" to nativeProofB64,
        "replay_witness_b64" to replayWitnessB64,
    )

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

internal const val SCCP_MAX_GROTH16_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024
internal const val SCCP_MAX_DESTINATION_ARTIFACT_BYTES =
    SCCP_MAX_GROTH16_ARTIFACT_BYTES + 64 * 1024
internal const val SCCP_MAX_DESTINATION_ARTIFACT_BASE64_BYTES = 22_544_384
internal const val SCCP_MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024
internal const val SCCP_MAX_REPLAY_WITNESS_BYTES = 16 * 1024
internal const val SCCP_MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024
internal const val SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME =
    "iroha_data_model::bridge::BridgeSccpDestinationProofV1"
internal const val SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME =
    "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1"
internal const val SCCP_REPLAY_WITNESS_SCHEMA_NAME =
    "iroha_data_model::bridge::sccp_replay::SccpSparseMerkleWitnessV1"
internal val SCCP_PROOF_REQUEST_SCHEMA_NAMES = setOf(
    "iroha_sccp::SccpGroth16Bn254ProofRequestV1",
    "iroha_sccp::SccpTonGroth16Bls12381ProofRequestV1",
)
internal fun validateCanonicalSccpNoritoBase64(
    value: String,
    field: String,
    maximum: Int,
    expectedSchemaName: String,
): ByteArray {
    require(value.isNotEmpty() && value == value.trim()) {
        "$field must be canonical padded base64"
    }
    require(value.length <= maximumBase64Length(maximum)) {
        "$field exceeds its canonical size bound"
    }
    val decoded = try {
        Base64.getDecoder().decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must be valid base64", ex)
    }
    require(decoded.isNotEmpty() && decoded.size <= maximum) {
        "$field exceeds its canonical size bound"
    }
    require(Base64.getEncoder().encodeToString(decoded) == value) {
        "$field must be canonical padded base64"
    }
    return validateCanonicalSccpNoritoBytes(
        decoded,
        field,
        maximum,
        setOf(expectedSchemaName),
    )
}

internal fun validateCanonicalSccpProofRequestNorito(
    value: ByteArray,
    field: String,
): ByteArray = validateCanonicalSccpNoritoBytes(
    value,
    field,
    SCCP_MAX_GROTH16_ARTIFACT_BYTES,
    SCCP_PROOF_REQUEST_SCHEMA_NAMES,
)

internal fun validateCanonicalSccpReplayWitnessBase64(
    value: String,
    field: String,
): ByteArray {
    val archive = validateCanonicalSccpNoritoBase64(
        value,
        field,
        SCCP_MAX_REPLAY_WITNESS_BYTES,
        SCCP_REPLAY_WITNESS_SCHEMA_NAME,
    )
    validateCanonicalSccpReplayWitnessArchive(archive, field)
    return archive
}

private fun validateCanonicalSccpNoritoBytes(
    decoded: ByteArray,
    field: String,
    maximum: Int,
    expectedSchemaNames: Set<String>,
): ByteArray {
    require(decoded.isNotEmpty() && decoded.size <= maximum) {
        "$field exceeds its canonical size bound"
    }
    val result = try {
        NoritoHeader.decode(decoded, null)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must contain a canonical Norito envelope", ex)
    }
    val header = result.header
    require(expectedSchemaNames.any { SchemaHash.hash16(it).contentEquals(header.schemaHash) }) {
        "$field schema hash does not match the closed SCCP type set"
    }
    require(header.compression == NoritoHeader.COMPRESSION_NONE) {
        "$field must use uncompressed canonical Norito"
    }
    val headerPadding = decoded.size - NoritoHeader.HEADER_LENGTH - header.payloadLength
    require(headerPadding == 0) {
        "$field must use the exact zero-padded SCCP Norito alignment"
    }
    require(header.encode().contentEquals(decoded.copyOfRange(0, NoritoHeader.HEADER_LENGTH))) {
        "$field contains a non-canonical Norito header"
    }
    header.validateChecksum(result.payload)
    return decoded.copyOf()
}

internal fun requireCanonicalSccpAuthority(value: String): String {
    val canonical = requireCanonicalI105Address(value, "authority")
    require(AccountAddress.detectI105Discriminant(canonical) == SccpV1.TAIRA_I105_DISCRIMINANT_V1) {
        "authority must use the canonical public Taira I105 discriminant"
    }
    return canonical
}

private fun validateCanonicalSccpReplayWitnessArchive(archive: ByteArray, field: String) {
    val payload = NoritoHeader.decode(archive, null).payload
    val cursor = SccpCompactCursor(payload)
    val expectedRoot = cursor.field("$field.expected_shard_root").requireSize(32, field)
    val priorRecordDigest = cursor.field("$field.prior_record_digest").requireSize(32, field)
    val siblingBitmap = cursor.field("$field.sibling_bitmap").requireSize(32, field)
    val siblingSequence = SccpCompactCursor(cursor.field("$field.siblings"))
    require(cursor.finished()) { "$field contains trailing fields" }
    val siblingCount = siblingSequence.u64("$field.siblings.count")
    require(siblingCount <= SccpReplayV1.DEPTH.toLong()) { "$field contains too many siblings" }
    val siblings = ArrayList<ByteArray>(siblingCount.toInt())
    repeat(siblingCount.toInt()) {
        siblings.add(siblingSequence.field("$field.sibling").requireSize(32, field))
    }
    require(siblingSequence.finished()) { "$field sibling sequence contains trailing bytes" }
    require(priorRecordDigest.all { it.toInt() == 0 }) {
        "$field must prove non-membership with an all-zero prior record digest"
    }
    SccpReplayV1.rootFromWitness(
        ByteArray(32) { 1 },
        null,
        SccpSparseMerkleWitnessV1(
            expectedRoot,
            priorRecordDigest,
            siblingBitmap,
            siblings,
        ),
    )
}

private fun ByteArray.requireSize(size: Int, field: String): ByteArray = also {
    require(it.size == size) { "$field contains a malformed fixed byte array" }
}

private class SccpCompactCursor(private val input: ByteArray) {
    private var offset = 0

    fun field(field: String): ByteArray {
        val length = compactLength(field)
        require(length <= Int.MAX_VALUE.toLong()) { "$field length exceeds the runtime bound" }
        return exact(length.toInt(), field)
    }

    fun u64(field: String): Long {
        val bytes = exact(8, field)
        require((bytes[7].toInt() and 0x80) == 0) { "$field exceeds the signed runtime bound" }
        var value = 0L
        for (index in bytes.indices) {
            value = value or ((bytes[index].toLong() and 0xff) shl (index * 8))
        }
        return value
    }

    fun finished(): Boolean = offset == input.size

    private fun compactLength(field: String): Long {
        var result = 0L
        var shift = 0
        while (true) {
            val value = exact(1, field)[0].toInt() and 0xff
            val chunk = value and 0x7f
            require(shift < 63 || chunk <= 1) { "$field compact length exceeds u64" }
            result = result or (chunk.toLong() shl shift)
            if (value and 0x80 == 0) {
                require(shift == 0 || chunk != 0) { "$field compact length is overlong" }
                return result
            }
            shift += 7
            require(shift < 64) { "$field compact length exceeds u64" }
        }
    }

    private fun exact(length: Int, field: String): ByteArray {
        require(length >= 0 && offset <= input.size - length) { "$field is truncated" }
        return input.copyOfRange(offset, offset + length).also { offset += length }
    }
}

private fun maximumBase64Length(maximumBytes: Int): Int = 4 * ((maximumBytes + 2) / 3)
