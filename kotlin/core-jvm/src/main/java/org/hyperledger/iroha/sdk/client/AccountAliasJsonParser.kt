package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Minimal JSON parser for account alias resolution payloads. */
object AccountAliasJsonParser {

    @JvmStatic
    fun parseResolution(payload: ByteArray): AccountAliasResolution {
        val root = expectObject(parse(payload, "account alias resolution"), "account alias resolution")
        return AccountAliasResolution(
            requiredString(root["alias"], "account alias resolution.alias"),
            requiredString(root["account_id"], "account alias resolution.account_id"),
            if (root.containsKey("index")) asOptionalLong(root["index"], "account alias resolution.index") else null,
            optionalString(root["source"]),
        )
    }

    private fun parse(payload: ByteArray?, context: String): Any? {
        check(payload != null && payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "$context returned a blank payload" }
        return JsonParser.parse(json)
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any?> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        return value as Map<String, Any?>
    }

    private fun requiredString(value: Any?, path: String): String {
        val string = optionalString(value)
        check(!string.isNullOrBlank()) { "$path must be a non-empty string" }
        return string.trim()
    }

    private fun optionalString(value: Any?): String? {
        if (value == null) return null
        return if (value is String) value else value.toString()
    }

    private fun asOptionalLong(value: Any?, path: String): Long? {
        if (value == null) return null
        return JsonNumbers.asLong(value, path)
    }
}
