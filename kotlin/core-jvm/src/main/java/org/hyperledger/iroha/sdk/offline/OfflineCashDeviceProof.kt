package org.hyperledger.iroha.sdk.offline

import java.util.Base64

/** Device attestation assertion carried by every cash-route mutation. */
class OfflineCashDeviceProof(
    val platform: String,
    val attestationKeyId: String,
    val challengeHashHex: String,
    val assertionBase64: String,
    val counter: Long? = null,
) {
    init {
        require(platform == "ios" || platform == "android" || platform == "android-keymint") {
            "platform must be a supported first-release value"
        }
        require(attestationKeyId.isExactNonEmptyProtocolString()) {
            "attestation_key_id must be an exact non-empty string"
        }
        require(challengeHashHex.isLowerHex32()) {
            "challenge_hash_hex must be 32-byte lowercase hex"
        }
        requireCanonicalNonEmptyBase64(assertionBase64, "assertion_base64")
        require(counter == null || counter >= 0) {
            "counter must be non-negative"
        }
    }

    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["platform"] = platform
        map["attestation_key_id"] = attestationKeyId
        map["challenge_hash_hex"] = challengeHashHex
        map["assertion_base64"] = assertionBase64
        if (counter != null) map["counter"] = counter
        return map
    }

    private fun String.isExactNonEmptyProtocolString(): Boolean =
        isNotEmpty() && trim() == this

    private fun String.isLowerHex32(): Boolean =
        length == 64 && all { it in '0'..'9' || it in 'a'..'f' }

    private fun requireCanonicalNonEmptyBase64(value: String, field: String): ByteArray {
        require(value.isNotEmpty() && value.trim() == value) { "$field must be canonical base64" }
        val decoded = try {
            Base64.getDecoder().decode(value)
        } catch (e: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical base64", e)
        }
        require(decoded.isNotEmpty() && Base64.getEncoder().encodeToString(decoded) == value) {
            "$field must be canonical base64"
        }
        return decoded
    }
}
