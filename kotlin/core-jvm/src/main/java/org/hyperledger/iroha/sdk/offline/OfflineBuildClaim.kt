package org.hyperledger.iroha.sdk.offline

/** Typed offline build-claim payload returned by Torii. */
class OfflineBuildClaim(
    val claimId: String,
    val nonce: String,
    platform: String,
    val appId: String,
    val buildNumber: Long,
    val issuedAtMs: Long,
    val expiresAtMs: Long,
    lineageScope: String?,
    val operatorSignatureHex: String,
) {
    /** Returns canonical wire token (`Apple` or `Android`). */
    val platform: String = normalizePlatform(platform)
    val lineageScope: String? = if (lineageScope.isNullOrBlank()) null else lineageScope.trim()

    /** Converts the claim to a JSON-ready map that can be embedded into receipts. */
    fun toJsonMap(): Map<String, Any> {
        val map = LinkedHashMap<String, Any>()
        map["claim_id"] = claimId
        map["nonce"] = nonce
        map["platform"] = this.platform
        map["app_id"] = appId
        map["build_number"] = buildNumber
        map["issued_at_ms"] = issuedAtMs
        map["expires_at_ms"] = expiresAtMs
        if (this.lineageScope != null) {
            map["lineage_scope"] = this.lineageScope
        }
        map["operator_signature"] = operatorSignatureHex
        return map
    }

    companion object {
        private fun normalizePlatform(platform: String): String {
            val normalized = platform.trim().lowercase()
            if (normalized == "apple" || normalized == "ios") return "Apple"
            if (normalized == "android") return "Android"
            throw IllegalArgumentException("platform must be either \"Apple\" or \"Android\"")
        }
    }
}
