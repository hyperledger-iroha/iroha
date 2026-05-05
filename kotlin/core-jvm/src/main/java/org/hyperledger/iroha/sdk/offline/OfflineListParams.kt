package org.hyperledger.iroha.sdk.offline

/** Query parameters for Torii offline listing endpoints. */
class OfflineListParams(
    val filter: String? = null,
    limit: Long? = null,
    offset: Long? = null,
    val sort: String? = null,
    val assetId: String? = null,
    certificateExpiresBeforeMs: Long? = null,
    certificateExpiresAfterMs: Long? = null,
    policyExpiresBeforeMs: Long? = null,
    policyExpiresAfterMs: Long? = null,
    val platformPolicy: PlatformPolicy? = null,
    val verdictIdHex: String? = null,
    val requireVerdict: Boolean = false,
    val onlyMissingVerdict: Boolean = false,
) {
    val limit: Long?
    val offset: Long?
    val certificateExpiresBeforeMs: Long?
    val certificateExpiresAfterMs: Long?
    val policyExpiresBeforeMs: Long?
    val policyExpiresAfterMs: Long?

    init {
        require(limit == null || limit >= 0) { "limit must be positive" }
        this.limit = limit
        require(offset == null || offset >= 0) { "offset must be positive" }
        this.offset = offset
        require(certificateExpiresBeforeMs == null || certificateExpiresBeforeMs >= 0) {
            "certificateExpiresBeforeMs must be positive"
        }
        this.certificateExpiresBeforeMs = certificateExpiresBeforeMs
        require(certificateExpiresAfterMs == null || certificateExpiresAfterMs >= 0) {
            "certificateExpiresAfterMs must be positive"
        }
        this.certificateExpiresAfterMs = certificateExpiresAfterMs
        require(policyExpiresBeforeMs == null || policyExpiresBeforeMs >= 0) {
            "policyExpiresBeforeMs must be positive"
        }
        this.policyExpiresBeforeMs = policyExpiresBeforeMs
        require(policyExpiresAfterMs == null || policyExpiresAfterMs >= 0) {
            "policyExpiresAfterMs must be positive"
        }
        this.policyExpiresAfterMs = policyExpiresAfterMs
        require(!(requireVerdict && onlyMissingVerdict)) {
            "`requireVerdict` cannot be combined with `onlyMissingVerdict`"
        }
        require(!(onlyMissingVerdict && !verdictIdHex.isNullOrBlank())) {
            "`verdictIdHex` cannot be combined with `onlyMissingVerdict`"
        }
    }

    /** Encodes the parameters into a string map suitable for query strings. */
    fun toQueryParameters(): Map<String, String> {
        val params = LinkedHashMap<String, String>()
        if (!filter.isNullOrBlank()) params["filter"] = filter
        if (this.limit != null) params["limit"] = this.limit.toString()
        if (this.offset != null) params["offset"] = this.offset.toString()
        if (!sort.isNullOrBlank()) params["sort"] = sort
        if (!assetId.isNullOrBlank()) params["asset_id"] = assetId.trim()
        if (this.certificateExpiresBeforeMs != null)
            params["certificate_expires_before_ms"] = this.certificateExpiresBeforeMs.toString()
        if (this.certificateExpiresAfterMs != null)
            params["certificate_expires_after_ms"] = this.certificateExpiresAfterMs.toString()
        if (this.policyExpiresBeforeMs != null)
            params["policy_expires_before_ms"] = this.policyExpiresBeforeMs.toString()
        if (this.policyExpiresAfterMs != null)
            params["policy_expires_after_ms"] = this.policyExpiresAfterMs.toString()
        if (platformPolicy != null) params["platform_policy"] = platformPolicy.slug
        if (!verdictIdHex.isNullOrBlank()) params["verdict_id_hex"] = verdictIdHex.lowercase()
        if (requireVerdict) params["require_verdict"] = "true"
        if (onlyMissingVerdict) params["only_missing_verdict"] = "true"
        return params
    }

    enum class PlatformPolicy(@JvmField val slug: String) {
        PLAY_INTEGRITY("play_integrity"),
        HMS_SAFETY_DETECT("hms_safety_detect"),
    }
}
