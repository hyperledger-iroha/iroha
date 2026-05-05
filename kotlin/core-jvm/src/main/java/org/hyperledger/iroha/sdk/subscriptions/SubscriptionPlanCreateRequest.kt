package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Request payload for `POST /v1/subscriptions/plans`. */
class SubscriptionPlanCreateRequest(
    authority: String,
    planId: String,
    plan: Map<String, Any>,
) {
    val authority: String = requireNonBlank(authority, "authority")
    val planId: String = requireNonBlank(planId, "plan_id")
    val plan: Map<String, Any> = LinkedHashMap(plan)

    fun toJsonMap(): Map<String, Any> = buildMap {
        put("authority", authority)
        put("plan_id", planId)
        put("plan", plan)
    }

    fun toJsonBytes(): ByteArray =
        JsonEncoder.encode(toJsonMap()).toByteArray(Charsets.UTF_8)

    companion object {
        /** Parse a plan from a JSON string. */
        @JvmStatic
        @Suppress("UNCHECKED_CAST")
        fun parsePlanJson(planJson: String): Map<String, Any> {
            val trimmed = planJson.trim()
            require(trimmed.isNotEmpty()) { "plan_json must not be blank" }
            val parsed = JsonParser.parse(trimmed)
            require(parsed is Map<*, *>) { "plan_json must encode a JSON object" }
            return parsed as Map<String, Any>
        }
    }
}

private fun requireNonBlank(value: String, field: String): String {
    val trimmed = value.trim()
    check(trimmed.isNotEmpty()) { "$field is required" }
    return trimmed
}
