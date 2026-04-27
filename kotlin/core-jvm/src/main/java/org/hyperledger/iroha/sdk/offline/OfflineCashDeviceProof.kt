package org.hyperledger.iroha.sdk.offline

/** Device attestation assertion carried by every cash-route mutation. */
class OfflineCashDeviceProof(
    val platform: String,
    val attestationKeyId: String,
    val challengeHashHex: String,
    val assertionBase64: String,
    val counter: Long? = null,
) {
    internal fun toJsonMap(): Map<String, Any?> {
        val map = LinkedHashMap<String, Any?>()
        map["platform"] = platform
        map["attestation_key_id"] = attestationKeyId
        map["challenge_hash_hex"] = challengeHashHex
        map["assertion_base64"] = assertionBase64
        if (counter != null) map["counter"] = counter
        return map
    }
}
