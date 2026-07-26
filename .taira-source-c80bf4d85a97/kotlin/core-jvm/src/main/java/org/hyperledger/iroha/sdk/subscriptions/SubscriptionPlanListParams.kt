package org.hyperledger.iroha.sdk.subscriptions

/** Query parameters for subscription plan listings. */
class SubscriptionPlanListParams(
    provider: String? = null,
    limit: Long? = null,
    offset: Long? = null,
) {
    val provider: String? = normalizeOptional(provider)

    val limit: Long? = limit?.also {
        require(it >= 0) { "limit must be non-negative" }
    }

    val offset: Long? = offset?.also {
        require(it >= 0) { "offset must be non-negative" }
    }

    fun toQueryParameters(): Map<String, String> = buildMap {
        this@SubscriptionPlanListParams.provider?.let { put("provider", it) }
        this@SubscriptionPlanListParams.limit?.let { put("limit", it.toString()) }
        this@SubscriptionPlanListParams.offset?.let { put("offset", it.toString()) }
    }
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
