package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.numeric.KotodamaQuantity
import org.hyperledger.iroha.sdk.numeric.NumericV1Codec

/** Request payload for subscription usage recording. */
class SubscriptionUsageRequest(
    authority: String,
    unitKey: String,
    delta: KotodamaQuantity,
    usageTriggerId: String? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val unitKey: String = requireNonBlank(unitKey, "unit_key")
    val delta: KotodamaQuantity = delta
    val usageTriggerId: String? = normalizeOptional(usageTriggerId)

    fun toJsonMap(): Map<String, Any> = buildMap {
        put("authority", authority)
        put("unit_key", unitKey)
        put("delta", NumericV1Codec.encodeQuantityJson(delta))
        this@SubscriptionUsageRequest.usageTriggerId?.let { put("usage_trigger_id", it) }
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
