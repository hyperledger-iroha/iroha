package org.hyperledger.iroha.sdk.offline

/** Request payload for issuing an operator-signed offline build claim. */
class OfflineBuildClaimIssueRequest(
    certificateIdHex: String,
    txIdHex: String,
    platform: String,
    appId: String? = null,
    buildNumber: Long? = null,
    issuedAtMs: Long? = null,
    expiresAtMs: Long? = null,
) {
    val certificateIdHex: String
    val txIdHex: String
    val platform: String
    val appId: String?
    val buildNumber: Long?
    val issuedAtMs: Long?
    val expiresAtMs: Long?

    init {
        this.certificateIdHex = OfflineHashLiteral.parseHex(
            requireNonBlank(certificateIdHex, "certificateIdHex"),
            "certificateIdHex",
        )
        this.txIdHex = OfflineHashLiteral.parseHex(
            requireNonBlank(txIdHex, "txIdHex"),
            "txIdHex",
        )
        this.platform = requireNonBlank(platform, "platform").lowercase()
        requireSupportedPlatform(this.platform)
        this.appId = normalizeOptional(appId)
        this.buildNumber = requireNonNegative(buildNumber, "buildNumber")
        this.issuedAtMs = requireNonNegative(issuedAtMs, "issuedAtMs")
        this.expiresAtMs = requireNonNegative(expiresAtMs, "expiresAtMs")
    }

    fun toJsonMap(): Map<String, Any> {
        val json = LinkedHashMap<String, Any>()
        json["certificate_id_hex"] = certificateIdHex
        json["tx_id_hex"] = txIdHex
        json["platform"] = platform
        if (appId != null) json["app_id"] = appId
        if (buildNumber != null) json["build_number"] = buildNumber
        if (issuedAtMs != null) json["issued_at_ms"] = issuedAtMs
        if (expiresAtMs != null) json["expires_at_ms"] = expiresAtMs
        return json
    }

    companion object {
        private fun requireNonBlank(value: String, name: String): String {
            val normalized = normalizeOptional(value)
            requireNotNull(normalized) { "$name must be provided" }
            return normalized
        }

        private fun normalizeOptional(value: String?): String? {
            if (value == null) return null
            val trimmed = value.trim()
            return trimmed.ifEmpty { null }
        }

        private fun requireNonNegative(value: Long?, name: String): Long? {
            if (value != null && value < 0) {
                throw IllegalArgumentException("$name must not be negative")
            }
            return value
        }

        private fun requireSupportedPlatform(platform: String) {
            require(platform == "apple" || platform == "android") {
                "platform must be either \"apple\" or \"android\""
            }
        }
    }
}
