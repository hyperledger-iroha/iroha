package org.hyperledger.iroha.sdk.offline

/** Optional per-receipt build-claim override for offline settlement submission. */
class OfflineSettlementBuildClaimOverride private constructor(
    private val txIdHex: String,
    private val appId: String?,
    private val buildNumber: Long?,
    private val issuedAtMs: Long?,
    private val expiresAtMs: Long?,
) {
    fun toJsonMap(): Map<String, Any> {
        val json = LinkedHashMap<String, Any>()
        json["tx_id_hex"] = txIdHex
        if (appId != null) json["app_id"] = appId
        if (buildNumber != null) json["build_number"] = buildNumber
        if (issuedAtMs != null) json["issued_at_ms"] = issuedAtMs
        if (expiresAtMs != null) json["expires_at_ms"] = expiresAtMs
        return json
    }

    class Builder {
        private var txIdHex: String? = null
        private var appId: String? = null
        private var buildNumber: Long? = null
        private var issuedAtMs: Long? = null
        private var expiresAtMs: Long? = null

        fun txIdHex(txIdHex: String?) = apply { this.txIdHex = txIdHex }
        fun appId(appId: String?) = apply { this.appId = appId }
        fun buildNumber(buildNumber: Long?) = apply { this.buildNumber = buildNumber }
        fun issuedAtMs(issuedAtMs: Long?) = apply { this.issuedAtMs = issuedAtMs }
        fun expiresAtMs(expiresAtMs: Long?) = apply { this.expiresAtMs = expiresAtMs }

        fun build(): OfflineSettlementBuildClaimOverride {
            val normalizedTxIdHex = requireNotNull(txIdHex?.trim()?.ifEmpty { null }) {
                "txIdHex must be provided"
            }
            val validatedTxIdHex = OfflineHashLiteral.parseHex(normalizedTxIdHex, "txIdHex")
            val normalizedAppId = appId?.trim()?.ifEmpty { null }
            require(buildNumber == null || buildNumber!! >= 0) { "buildNumber must not be negative" }
            require(issuedAtMs == null || issuedAtMs!! >= 0) { "issuedAtMs must not be negative" }
            require(expiresAtMs == null || expiresAtMs!! >= 0) { "expiresAtMs must not be negative" }
            return OfflineSettlementBuildClaimOverride(
                validatedTxIdHex, normalizedAppId, buildNumber, issuedAtMs, expiresAtMs
            )
        }
    }

    companion object {
        @JvmStatic
        fun builder() = Builder()
    }
}
