package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Request payload for `POST /v1/subscriptions`. */
class SubscriptionCreateRequest(
    authority: String,
    privateKey: String,
    subscriptionId: String,
    planId: String,
    billingTriggerId: String? = null,
    usageTriggerId: String? = null,
    firstChargeMs: Long? = null,
    val grantUsageToProvider: Boolean? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val privateKey: String = requireNonBlank(privateKey, "private_key")
    val subscriptionId: String = requireNonBlank(subscriptionId, "subscription_id")
    val planId: String = requireNonBlank(planId, "plan_id")
    val billingTriggerId: String? = normalizeOptional(billingTriggerId)
    val usageTriggerId: String? = normalizeOptional(usageTriggerId)
    val firstChargeMs: Long? = firstChargeMs?.also {
        require(it >= 0) { "firstChargeMs must be non-negative" }
    }

    fun toJsonMap(): Map<String, Any> = buildMap {
        put("authority", authority)
        put("private_key", privateKey)
        put("subscription_id", subscriptionId)
        put("plan_id", planId)
        this@SubscriptionCreateRequest.billingTriggerId?.let { put("billing_trigger_id", it) }
        this@SubscriptionCreateRequest.usageTriggerId?.let { put("usage_trigger_id", it) }
        this@SubscriptionCreateRequest.firstChargeMs?.let { put("first_charge_ms", it) }
        grantUsageToProvider?.let { put("grant_usage_to_provider", it) }
    }

    fun toJsonBytes(): ByteArray =
        JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)
}

private fun requireNonBlank(value: String, field: String): String {
    val trimmed = value.trim()
    check(trimmed.isNotEmpty()) { "$field is required" }
    return trimmed
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
