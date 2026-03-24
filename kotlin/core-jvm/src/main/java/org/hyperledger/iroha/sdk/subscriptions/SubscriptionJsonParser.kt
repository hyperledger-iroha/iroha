package org.hyperledger.iroha.sdk.subscriptions

import org.hyperledger.iroha.sdk.client.JsonParser

/** JSON parser for subscription Torii endpoints. */
object SubscriptionJsonParser {

    @JvmStatic
    fun parsePlanCreateResponse(payload: ByteArray): SubscriptionPlanCreateResponse {
        val obj = expectObject(parse(payload), "root")
        return SubscriptionPlanCreateResponse(
            ok = asBoolean(obj["ok"], "ok"),
            planId = asString(obj["plan_id"], "plan_id"),
            txHashHex = asString(obj["tx_hash_hex"], "tx_hash_hex"),
        )
    }

    @JvmStatic
    fun parsePlanList(payload: ByteArray): SubscriptionPlanListResponse {
        val obj = expectObject(parse(payload), "root")
        val items = asArrayOrEmpty(obj["items"], "items")
        val total = asLongOrDefault(obj["total"], "total", items.size.toLong())
        val parsed = items.mapIndexed { i, item ->
            val path = "items[$i]"
            val entry = expectObject(item, path)
            SubscriptionPlanListResponse.SubscriptionPlanListItem(
                planId = asString(entry["plan_id"], "$path.plan_id"),
                plan = expectObject(entry["plan"], "$path.plan"),
            )
        }
        return SubscriptionPlanListResponse(parsed, total)
    }

    @JvmStatic
    fun parseSubscriptionCreateResponse(payload: ByteArray): SubscriptionCreateResponse {
        val obj = expectObject(parse(payload), "root")
        return SubscriptionCreateResponse(
            ok = asBoolean(obj["ok"], "ok"),
            subscriptionId = asString(obj["subscription_id"], "subscription_id"),
            billingTriggerId = asString(obj["billing_trigger_id"], "billing_trigger_id"),
            usageTriggerId = asOptionalString(obj["usage_trigger_id"], "usage_trigger_id"),
            firstChargeMs = asLong(obj["first_charge_ms"], "first_charge_ms"),
            txHashHex = asString(obj["tx_hash_hex"], "tx_hash_hex"),
        )
    }

    @JvmStatic
    fun parseSubscriptionList(payload: ByteArray): SubscriptionListResponse {
        val obj = expectObject(parse(payload), "root")
        val items = asArrayOrEmpty(obj["items"], "items")
        val total = asLongOrDefault(obj["total"], "total", items.size.toLong())
        val parsed = items.mapIndexed { i, item ->
            parseSubscriptionRecord(item, "items[$i]")
        }
        return SubscriptionListResponse(parsed, total)
    }

    @JvmStatic
    fun parseSubscriptionRecord(payload: ByteArray): SubscriptionListResponse.SubscriptionRecord =
        parseSubscriptionRecord(parse(payload), "root")

    @JvmStatic
    fun parseActionResponse(payload: ByteArray): SubscriptionActionResponse {
        val obj = expectObject(parse(payload), "root")
        return SubscriptionActionResponse(
            ok = asBoolean(obj["ok"], "ok"),
            subscriptionId = asString(obj["subscription_id"], "subscription_id"),
            txHashHex = asString(obj["tx_hash_hex"], "tx_hash_hex"),
        )
    }

    private fun parse(payload: ByteArray): Any {
        val json = String(payload, Charsets.UTF_8).trim()
        check(json.isNotEmpty()) { "Empty JSON payload" }
        return JsonParser.parse(json) ?: throw IllegalStateException("JSON payload parsed to null")
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any> {
        check(value is Map<*, *>) { "$path is not a JSON object" }
        return value as Map<String, Any>
    }

    @Suppress("UNCHECKED_CAST")
    private fun asArray(value: Any?, path: String): List<Any> {
        check(value is List<*>) { "$path is not a JSON array" }
        return value as List<Any>
    }

    private fun asArrayOrEmpty(value: Any?, path: String): List<Any> =
        if (value == null) emptyList() else asArray(value, path)

    private fun asString(value: Any?, path: String): String {
        checkNotNull(value) { "$path is missing" }
        return if (value is String) value else value.toString()
    }

    private fun asOptionalString(value: Any?, path: String): String? {
        if (value == null) return null
        check(value is String) { "$path must be a string when present" }
        return value
    }

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path is missing" }
        return value
    }

    private fun asLong(value: Any?, path: String): Long {
        check(value is Number) { "$path is not a number" }
        if (value is Double || value is Float) {
            val numeric = value.toDouble()
            check(numeric % 1 == 0.0) { "$path must be an integer" }
            return numeric.toLong()
        }
        return value.toLong()
    }

    private fun asLongOrDefault(value: Any?, path: String, fallback: Long): Long =
        if (value == null) fallback else asLong(value, path)

    private fun parseSubscriptionRecord(value: Any?, path: String): SubscriptionListResponse.SubscriptionRecord {
        val entry = expectObject(value, path)
        return SubscriptionListResponse.SubscriptionRecord(
            subscriptionId = asString(entry["subscription_id"], "$path.subscription_id"),
            subscription = expectObject(entry["subscription"], "$path.subscription"),
            invoice = asOptionalObject(entry["invoice"], "$path.invoice"),
            plan = asOptionalObject(entry["plan"], "$path.plan"),
        )
    }

    private fun asOptionalObject(value: Any?, path: String): Map<String, Any>? =
        if (value == null) null else expectObject(value, path)
}
