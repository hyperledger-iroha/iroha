package org.hyperledger.iroha.sdk.client

import java.util.Base64
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.tx.norito.NoritoJavaCodecAdapter

/**
 * Unsigned verifying-key registry transaction prepared by Torii.
 *
 * SDK [org.hyperledger.iroha.sdk.crypto.Signer] implementations already apply
 * Iroha's prehash, so pass [transactionPayloadBytes] to `Signer.sign`. Use
 * [signingMessageBytes] only with a raw signing primitive that signs an
 * already-prehashed message. Attach the signature to the decoded payload and
 * submit the resulting signed transaction through the standard ingress.
 */
class VerifyingKeyTransactionDraft internal constructor(
    @JvmField val submitted: Boolean,
    @JvmField val transactionPayloadB64: String,
    @JvmField val signingMessageB64: String,
) {
    /** Decode the canonical transaction payload returned by Torii. */
    fun transactionPayloadBytes(): ByteArray = Base64.getDecoder().decode(transactionPayloadB64)

    /** Decode the exact 32-byte message for a raw signer that does not apply Iroha's prehash. */
    fun signingMessageBytes(): ByteArray = Base64.getDecoder().decode(signingMessageB64)

    override fun equals(other: Any?): Boolean =
        other is VerifyingKeyTransactionDraft &&
            submitted == other.submitted &&
            transactionPayloadB64 == other.transactionPayloadB64 &&
            signingMessageB64 == other.signingMessageB64

    override fun hashCode(): Int {
        var result = submitted.hashCode()
        result = 31 * result + transactionPayloadB64.hashCode()
        result = 31 * result + signingMessageB64.hashCode()
        return result
    }
}

/** Strict parser for the first-release verifying-key transaction draft envelope. */
internal object VerifyingKeyTransactionDraftParser {
    internal fun parseRegister(
        bytes: ByteArray,
        expectedChainId: String,
        request: Map<String, Any>,
    ): VerifyingKeyTransactionDraft =
        parse(bytes, expectedChainId, request, VerifyingKeyDraftOperation.REGISTER)

    internal fun parseUpdate(
        bytes: ByteArray,
        expectedChainId: String,
        request: Map<String, Any>,
    ): VerifyingKeyTransactionDraft =
        parse(bytes, expectedChainId, request, VerifyingKeyDraftOperation.UPDATE)

    private fun parse(
        bytes: ByteArray,
        expectedChainId: String,
        request: Map<String, Any>,
        operation: VerifyingKeyDraftOperation,
    ): VerifyingKeyTransactionDraft {
        val value = parseObject(bytes)
        val unknown = value.keys.firstOrNull { it !in FIELDS }
        require(unknown == null) {
            "verifying-key draft contains unknown or retired field `$unknown`"
        }
        val missing = FIELDS.firstOrNull { it !in value }
        require(missing == null) {
            "verifying-key draft is missing required field `$missing`"
        }
        val submitted = value["submitted"] as? Boolean
            ?: throw IllegalArgumentException("submitted must be a boolean")
        require(!submitted) { "verifying-key draft must be unsigned and unsubmitted" }
        val transactionPayloadB64 = exactString(value, "transaction_payload_b64")
        val signingMessageB64 = exactString(value, "signing_message_b64")
        val transactionPayload = decodeCanonicalBase64(
            transactionPayloadB64,
            "transaction_payload_b64",
            MAX_TRANSACTION_PAYLOAD_BYTES,
        )
        val signingMessage = decodeCanonicalBase64(
            signingMessageB64,
            "signing_message_b64",
            SIGNING_MESSAGE_BYTES,
            SIGNING_MESSAGE_BYTES,
        )
        try {
            NoritoJavaCodecAdapter.validateCanonicalTransactionPayload(transactionPayload)
        } catch (ex: Exception) {
            throw IllegalArgumentException(
                "transaction_payload_b64 must contain one canonical transaction payload",
                ex,
            )
        }
        VerifyingKeyDraftBinding.validate(
            transactionPayload,
            expectedChainId,
            request,
            operation,
        )
        require(signingMessage.contentEquals(IrohaHash.prehash(transactionPayload))) {
            "signing_message_b64 must be the exact transaction-payload prehash"
        }
        return VerifyingKeyTransactionDraft(
            submitted,
            transactionPayloadB64,
            signingMessageB64,
        )
    }

    @Suppress("UNCHECKED_CAST")
    private fun parseObject(bytes: ByteArray): Map<String, Any?> {
        require(bytes.isNotEmpty()) { "verifying-key draft returned an empty payload" }
        val text = String(bytes, Charsets.UTF_8)
        require(text.toByteArray(Charsets.UTF_8).contentEquals(bytes)) {
            "verifying-key draft must be UTF-8 JSON"
        }
        val value = JsonParser.parse(text)
        require(value is Map<*, *> && value.keys.all { it is String }) {
            "verifying-key draft must be a JSON object"
        }
        return value as Map<String, Any?>
    }

    private fun exactString(value: Map<String, Any?>, field: String): String {
        val text = value[field] as? String
            ?: throw IllegalArgumentException("$field must be a string")
        require(text.isNotEmpty() && text == text.trim()) {
            "$field must be canonical non-empty text"
        }
        return text
    }

    private fun decodeCanonicalBase64(
        value: String,
        field: String,
        maximumBytes: Int,
        exactBytes: Int? = null,
    ): ByteArray {
        require(value.length <= 4 * ((maximumBytes + 2) / 3)) {
            "$field exceeds its size bound"
        }
        val decoded = try {
            Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical padded base64", ex)
        }
        require(
            decoded.isNotEmpty() &&
                Base64.getEncoder().encodeToString(decoded) == value,
        ) {
            "$field must be canonical non-empty padded base64"
        }
        if (exactBytes != null) {
            require(decoded.size == exactBytes) {
                "$field must contain exactly $exactBytes bytes"
            }
        }
        return decoded
    }

    private const val MAX_TRANSACTION_PAYLOAD_BYTES = 16 * 1024 * 1024
    private const val SIGNING_MESSAGE_BYTES = 32
    private val FIELDS = setOf(
        "submitted",
        "transaction_payload_b64",
        "signing_message_b64",
    )
}
