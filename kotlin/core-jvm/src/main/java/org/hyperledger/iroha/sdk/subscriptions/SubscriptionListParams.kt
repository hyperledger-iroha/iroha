package org.hyperledger.iroha.sdk.subscriptions

/** Query parameters for subscription listings. */
class SubscriptionListParams(
    ownedBy: String? = null,
    provider: String? = null,
    status: SubscriptionStatus? = null,
    limit: Long? = null,
    offset: Long? = null,
) {
    val ownedBy: String? = normalizeOptional(ownedBy)
    val provider: String? = normalizeOptional(provider)
    val status: SubscriptionStatus? = status

    val limit: Long? = limit?.also {
        require(it >= 0) { "limit must be non-negative" }
    }

    val offset: Long? = offset?.also {
        require(it >= 0) { "offset must be non-negative" }
    }

    fun toQueryParameters(): Map<String, String> = buildMap {
        this@SubscriptionListParams.ownedBy?.let { put("owned_by", it) }
        this@SubscriptionListParams.provider?.let { put("provider", it) }
        status?.let { put("status", it.slug) }
        this@SubscriptionListParams.limit?.let { put("limit", it.toString()) }
        this@SubscriptionListParams.offset?.let { put("offset", it.toString()) }
    }
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
