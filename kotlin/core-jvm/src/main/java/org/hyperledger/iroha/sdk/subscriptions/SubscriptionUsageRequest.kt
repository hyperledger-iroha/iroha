package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Request payload for subscription usage recording. */
class SubscriptionUsageRequest(
    authority: String,
    privateKey: String,
    unitKey: String,
    delta: String,
    usageTriggerId: String? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val privateKey: String = requireNonBlank(privateKey, "private_key")
    val unitKey: String = requireNonBlank(unitKey, "unit_key")
    val delta: String = requireNumericLiteral(delta, "delta")
    val usageTriggerId: String? = normalizeOptional(usageTriggerId)

    fun toJsonMap(): Map<String, Any> = buildMap {
        put("authority", authority)
        put("private_key", privateKey)
        put("unit_key", unitKey)
        put("delta", delta)
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

private fun requireNumericLiteral(value: String, field: String): String {
    val trimmed = requireNonBlank(value, field)
    var index = 0
    val first = trimmed[0]
    if (first == '+' || first == '-') {
        require(first != '-') { "$field must be non-negative" }
        index = 1
    }
    require(index < trimmed.length) { "$field must be numeric" }
    var seenDot = false
    var seenDigit = false
    for (i in index until trimmed.length) {
        val ch = trimmed[i]
        if (ch == '.') {
            require(!seenDot) { "$field must be numeric" }
            seenDot = true
            continue
        }
        require(ch in '0'..'9') { "$field must be numeric" }
        seenDigit = true
    }
    require(seenDigit) { "$field must be numeric" }
    return trimmed
}

private fun normalizeOptional(value: String?): String? {
    if (value == null) return null
    val trimmed = value.trim()
    return trimmed.ifEmpty { null }
}
