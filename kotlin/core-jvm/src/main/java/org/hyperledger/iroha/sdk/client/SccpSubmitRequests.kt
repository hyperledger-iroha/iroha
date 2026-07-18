package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.AccountAddressException
import org.hyperledger.iroha.sdk.address.requireCanonicalI105Address
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.SchemaHash
import org.hyperledger.iroha.sdk.sccp.SccpV1
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/** Exact request payload for POST /v1/bridge/proofs/submit. */
class SccpDestinationProofSubmitRequest(
    authority: String,
    destinationProofB64: String,
    feePayment: FeePaymentIntent,
    signatureB64: String? = null,
    transactionPayloadB64: String? = null,
    creationTimeMs: Long? = null,
) {
    val authority: String = requireCanonicalSccpAuthority(authority)
    val feePayment: FeePaymentIntent = feePayment
    val signatureB64: String? = normalizeOptionalSignature(signatureB64)
    val transactionPayloadB64: String? = normalizeOptionalTransactionPayload(
        transactionPayloadB64,
        creationTimeMs,
        this.authority,
        this.feePayment,
    )
    val destinationProofB64: String = destinationProofB64.also {
        validateCanonicalSccpNoritoBase64(
            it,
            "destinationProofB64",
            SCCP_MAX_DESTINATION_ARTIFACT_BYTES,
            SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME,
        )
    }
    val creationTimeMs: Long? = creationTimeMs?.also {
        require(it > 0) { "creationTimeMs must be positive" }
    }

    init {
        validateSccpDetachedSigningState(signatureB64, transactionPayloadB64, this.creationTimeMs)
    }

    /** Exact JSON object accepted by Torii; route overrides are unrepresentable. */
    fun toJsonMap(): Map<String, Any> = linkedMapOf<String, Any>(
        "authority" to authority,
        "fee_payment" to feePayment.toJsonMap(),
        "destination_proof_b64" to destinationProofB64,
    ).also { output ->
        signatureB64?.let { output["signature_b64"] = it }
        transactionPayloadB64?.let { output["transaction_payload_b64"] = it }
        creationTimeMs?.let { output["creation_time_ms"] = it }
    }

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

/** Exact native-proof request payload for POST /v1/bridge/messages. */
class SccpNativeMessageSubmitRequest(
    authority: String,
    nativeProofB64: String,
    feePayment: FeePaymentIntent,
    signatureB64: String? = null,
    transactionPayloadB64: String? = null,
    creationTimeMs: Long? = null,
) {
    val authority: String = requireCanonicalSccpAuthority(authority)
    val feePayment: FeePaymentIntent = feePayment
    val signatureB64: String? = normalizeOptionalSignature(signatureB64)
    val transactionPayloadB64: String? = normalizeOptionalTransactionPayload(
        transactionPayloadB64,
        creationTimeMs,
        this.authority,
        this.feePayment,
    )
    val nativeProofB64: String = nativeProofB64.also {
        validateCanonicalSccpNoritoBase64(
            it,
            "nativeProofB64",
            SCCP_MAX_NATIVE_PROOF_BYTES,
            SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME,
        )
    }
    val creationTimeMs: Long? = creationTimeMs?.also {
        require(it > 0) { "creationTimeMs must be positive" }
    }

    init {
        validateSccpDetachedSigningState(signatureB64, transactionPayloadB64, this.creationTimeMs)
    }

    /** Exact JSON object accepted by Torii; settlement selectors are unrepresentable. */
    fun toJsonMap(): Map<String, Any> = linkedMapOf<String, Any>(
        "authority" to authority,
        "fee_payment" to feePayment.toJsonMap(),
        "native_proof_b64" to nativeProofB64,
    ).also { output ->
        signatureB64?.let { output["signature_b64"] = it }
        transactionPayloadB64?.let { output["transaction_payload_b64"] = it }
        creationTimeMs?.let { output["creation_time_ms"] = it }
    }

    fun toJsonBytes(): ByteArray = JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

private const val SCCP_MAX_DESTINATION_ARTIFACT_BYTES = 16 * 1024 * 1024 + 64 * 1024
private const val SCCP_MAX_NATIVE_PROOF_BYTES = 16 * 1024 * 1024
internal const val SCCP_MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024
private const val SCCP_MAX_DETACHED_SIGNATURE_BYTES = 16 * 1024
internal const val SCCP_DESTINATION_ARTIFACT_SCHEMA_NAME =
    "iroha_sccp::SccpGroth16Bn254ProofArtifactV1"
internal const val SCCP_NATIVE_INBOUND_PROOF_SCHEMA_NAME =
    "iroha_sccp::native_admission::SccpNativeInboundMessageProofV1"
private val SCCP_TRANSACTION_CODEC = NoritoJavaCodecAdapter()

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
    val result = try {
        NoritoHeader.decode(decoded, SchemaHash.hash16(expectedSchemaName))
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must contain a canonical Norito envelope", ex)
    }
    val header = result.header
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
    return decoded
}

internal fun requireCanonicalSccpAuthority(value: String): String {
    val canonical = requireCanonicalI105Address(value, "authority")
    require(AccountAddress.detectI105Discriminant(canonical) == SccpV1.TAIRA_I105_DISCRIMINANT_V1) {
        "authority must use the canonical public Taira I105 discriminant"
    }
    return canonical
}

internal fun normalizeOptionalSignature(value: String?): String? {
    if (value == null) return null
    val decoded = decodeCanonicalBase64(
        value,
        "signature_b64",
        SCCP_MAX_DETACHED_SIGNATURE_BYTES,
    )
    require(decoded.any { it.toInt() != 0 }) {
        "signature_b64 must contain one admitted nonzero signature payload"
    }
    return value
}

internal fun validateSccpDetachedSigningState(
    signatureB64: String?,
    transactionPayloadB64: String?,
    creationTimeMs: Long?,
) {
    when {
        signatureB64 == null && transactionPayloadB64 == null -> Unit
        signatureB64 != null && transactionPayloadB64 != null -> require(creationTimeMs != null && creationTimeMs > 0) {
            "signed SCCP submission requires an explicit positive creation_time_ms"
        }
        else -> throw IllegalArgumentException(
            "SCCP preparation requires neither signature_b64 nor transaction_payload_b64; signed submission requires both",
        )
    }
}

internal fun normalizeOptionalTransactionPayload(
    value: String?,
    creationTimeMs: Long?,
    expectedAuthority: String,
    expectedFeePayment: FeePaymentIntent,
): String? {
    if (value == null) return null
    val bytes = decodeCanonicalBase64(value, "transaction_payload_b64", SCCP_MAX_TRANSACTION_PAYLOAD_BYTES)
    val payload = try {
        SCCP_TRANSACTION_CODEC.decodeTransaction(bytes)
    } catch (ex: Exception) {
        throw IllegalArgumentException(
            "transaction_payload_b64 must contain one canonical transaction payload",
            ex,
        )
    }
    val canonical = try {
        SCCP_TRANSACTION_CODEC.encodeTransaction(payload)
    } catch (ex: Exception) {
        throw IllegalArgumentException("transaction_payload_b64 could not be canonically re-encoded", ex)
    }
    require(canonical.contentEquals(bytes)) { "transaction_payload_b64 is not canonical" }
    require(sameCanonicalAccountId(payload.authority, expectedAuthority)) {
        "transaction payload authority does not match authority"
    }
    require(expectedFeePayment.hasSamePayerAndGasBound(payload.feePayment)) {
        "transaction payload changed the requested payer, sponsor revision, or gas bound"
    }
    if (creationTimeMs != null) {
        require(payload.creationTimeMs == creationTimeMs) {
            "transaction payload creation time does not match creation_time_ms"
        }
    }
    return value
}

private fun sameCanonicalAccountId(left: String, right: String): Boolean = try {
    // AccountId wire identity is domainless and excludes its I105 display discriminant.
    val leftBytes = AccountAddress.parseEncodedIgnoringCurveSupport(left, null).address.canonicalBytes
    val rightBytes = AccountAddress.parseEncodedIgnoringCurveSupport(right, null).address.canonicalBytes
    leftBytes.contentEquals(rightBytes)
} catch (ex: AccountAddressException) {
    throw IllegalArgumentException("transaction payload authority must be canonical I105", ex)
}

internal fun decodeCanonicalBase64(value: String, field: String, maximum: Int): ByteArray {
    require(value.isNotEmpty() && value == value.trim()) { "$field must be canonical padded base64" }
    require(value.length <= maximumBase64Length(maximum)) { "$field exceeds its canonical size bound" }
    val decoded = try {
        Base64.getDecoder().decode(value)
    } catch (ex: IllegalArgumentException) {
        throw IllegalArgumentException("$field must be valid base64", ex)
    }
    require(decoded.isNotEmpty() && decoded.size <= maximum) { "$field exceeds its canonical size bound" }
    require(Base64.getEncoder().encodeToString(decoded) == value) { "$field must be canonical padded base64" }
    return decoded
}

private fun maximumBase64Length(maximumBytes: Int): Int = 4 * ((maximumBytes + 2) / 3)
