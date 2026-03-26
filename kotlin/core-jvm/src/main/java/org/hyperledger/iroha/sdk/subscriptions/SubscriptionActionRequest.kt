package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder

/** Request payload for subscription action endpoints (pause/resume/cancel/charge-now). */
class SubscriptionActionRequest(
    authority: String,
    chargeAtMs: Long? = null,
    val cancelMode: CancelMode? = null,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val chargeAtMs: Long? = chargeAtMs?.also {
        require(it >= 0) { "chargeAtMs must be non-negative" }
    }

    fun toJsonMap(): Map<String, Any> = buildMap {
        put("authority", authority)
        chargeAtMs?.let { put("charge_at_ms", it) }
        cancelMode?.let { put("cancel_mode", it.value) }
    }

    fun toJsonBytes(): ByteArray =
        JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)

    enum class CancelMode(val value: String) {
        IMMEDIATE("immediate"),
        PERIOD_END("period_end"),
    }
}

private fun requireNonBlank(value: String, field: String): String {
    val trimmed = value.trim()
    check(trimmed.isNotEmpty()) { "$field is required" }
    return trimmed
}
