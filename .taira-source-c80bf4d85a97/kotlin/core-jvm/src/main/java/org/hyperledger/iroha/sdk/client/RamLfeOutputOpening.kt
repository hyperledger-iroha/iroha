package org.hyperledger.iroha.sdk.client

import java.util.LinkedHashMap

/** Canonical payload signed by an external RAM-LFE output-opening authority. */
class RamLfeOutputOpeningPayload(
    @JvmField val programId: String,
    @JvmField val inputCiphertextHash: String,
    @JvmField val outputCiphertextHash: String,
    @JvmField val parameterDigest: String,
    @JvmField val evaluationKeyDigest: String,
    @JvmField val openedOutputHash: String,
    @JvmField val openedAtMs: Long,
    @JvmField val expiresAtMs: Long?,
) {
    fun toJsonMap(): Map<String, Any> {
        val payload = LinkedHashMap<String, Any>()
        payload["program_id"] = HttpClientTransport.normalizeNonBlank(programId, "opening.payload.programId")
        payload["input_ciphertext_hash"] =
            HttpClientTransport.normalizeHex32(inputCiphertextHash, "opening.payload.inputCiphertextHash")
        payload["output_ciphertext_hash"] =
            HttpClientTransport.normalizeHex32(outputCiphertextHash, "opening.payload.outputCiphertextHash")
        payload["parameter_digest"] =
            HttpClientTransport.normalizeHex32(parameterDigest, "opening.payload.parameterDigest")
        payload["evaluation_key_digest"] =
            HttpClientTransport.normalizeHex32(evaluationKeyDigest, "opening.payload.evaluationKeyDigest")
        payload["opened_output_hash"] =
            HttpClientTransport.normalizeHex32(openedOutputHash, "opening.payload.openedOutputHash")
        payload["opened_at_ms"] = openedAtMs
        if (expiresAtMs != null) {
            payload["expires_at_ms"] = expiresAtMs
        }
        return payload
    }
}

/** Externally attested opening of a RAM-LFE encrypted output. */
class RamLfeOutputOpening(
    @JvmField val payload: RamLfeOutputOpeningPayload,
    @JvmField val signature: String,
) {
    fun toJsonMap(): Map<String, Any> {
        val opening = LinkedHashMap<String, Any>()
        opening["payload"] = payload.toJsonMap()
        opening["signature"] = HttpClientTransport.normalizeEvenLengthHex(signature, "opening.signature")
        return opening
    }
}
