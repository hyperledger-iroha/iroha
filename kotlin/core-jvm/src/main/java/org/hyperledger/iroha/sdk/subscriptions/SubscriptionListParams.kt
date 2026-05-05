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
    val status: String? = status?.slug

    val limit: Long? = limit?.also {
        require(it >= 0) { "limit must be non-negative" }
    }

    val offset: Long? = offset?.also {
        require(it >= 0) { "offset must be non-negative" }
    }

    /** Secondary constructor accepting raw status string. */
    constructor(
        ownedBy: String?,
        provider: String?,
        statusString: String?,
        limit: Long?,
        offset: Long?,
        @Suppress("UNUSED_PARAMETER") marker: Unit = Unit,
    ) : this(
        ownedBy = ownedBy,
        provider = provider,
        status = statusString?.trim()?.takeIf { it.isNotEmpty() }?.let { SubscriptionStatus.fromString(it) },
        limit = limit,
        offset = offset,
    )

    fun toQueryParameters(): Map<String, String> = buildMap {
        this@SubscriptionListParams.ownedBy?.let { put("owned_by", it) }
        this@SubscriptionListParams.provider?.let { put("provider", it) }
        status?.let { put("status", it) }
        this@SubscriptionListParams.limit?.let { put("limit", it.toString()) }
        this@SubscriptionListParams.offset?.let { put("offset", it.toString()) }
    }
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
