package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Minimal JSON parser for RAM-LFE program-policy, execute, and verify payloads. */
object RamLfeJsonParser {

    @JvmStatic
    fun parsePolicyList(payload: ByteArray): RamLfeProgramPolicyListResponse {
        val root = expectObject(parse(payload, "ram-lfe program policy list"), "ram-lfe program policy list")
        val itemValues = asArrayOrEmpty(root["items"], "ram-lfe program policy list.items")
        val items = ArrayList<RamLfeProgramPolicySummary>(itemValues.size)
        for (i in itemValues.indices) {
            val item = expectObject(itemValues[i], "ram-lfe program policy list.items[$i]")
            items.add(
                RamLfeProgramPolicySummary(
                    requiredString(item["program_id"], "ram-lfe program policy list.items[$i].program_id"),
                    requiredString(item["owner"], "ram-lfe program policy list.items[$i].owner"),
                    item["active"] == true,
                    requiredString(item["resolver_public_key"], "ram-lfe program policy list.items[$i].resolver_public_key"),
                    requiredString(item["backend"], "ram-lfe program policy list.items[$i].backend"),
                    normalizedMode(requiredString(item["verification_mode"], "ram-lfe program policy list.items[$i].verification_mode")),
                    optionalString(item["input_encryption"]),
                    optionalString(item["input_encryption_public_parameters"]),
                    if (item["input_encryption_public_parameters_decoded"] == null) null
                    else parseBfvPublicParameters(
                        expectObject(item["input_encryption_public_parameters_decoded"],
                            "ram-lfe program policy list.items[$i].input_encryption_public_parameters_decoded"),
                        "ram-lfe program policy list.items[$i].input_encryption_public_parameters_decoded"
                    ),
                    optionalString(item["note"])
                )
            )
        }
        val total = if (root.containsKey("total"))
            asLong(root["total"], "ram-lfe program policy list.total")
        else items.size.toLong()
        return RamLfeProgramPolicyListResponse(total, items)
    }

    @JvmStatic
    fun parseExecuteResponse(payload: ByteArray): RamLfeExecuteResponse {
        val root = expectObject(parse(payload, "ram-lfe execute response"), "ram-lfe execute response")
        return RamLfeExecuteResponse(
            requiredString(root["program_id"], "ram-lfe execute response.program_id"),
            requiredString(root["opaque_hash"], "ram-lfe execute response.opaque_hash"),
            requiredString(root["receipt_hash"], "ram-lfe execute response.receipt_hash"),
            canonicalizeHex(requiredString(root["output_hex"], "ram-lfe execute response.output_hex"), "ram-lfe execute response.output_hex"),
            requiredString(root["output_hash"], "ram-lfe execute response.output_hash"),
            requiredString(root["associated_data_hash"], "ram-lfe execute response.associated_data_hash"),
            asLong(root["executed_at_ms"], "ram-lfe execute response.executed_at_ms"),
            if (root.containsKey("expires_at_ms")) asOptionalLong(root["expires_at_ms"], "ram-lfe execute response.expires_at_ms") else null,
            requiredString(root["backend"], "ram-lfe execute response.backend"),
            normalizedMode(requiredString(root["verification_mode"], "ram-lfe execute response.verification_mode")),
            expectObject(root["receipt"], "ram-lfe execute response.receipt")
        )
    }

    @JvmStatic
    fun parseReceiptVerifyResponse(payload: ByteArray): RamLfeReceiptVerifyResponse {
        val root = expectObject(parse(payload, "ram-lfe receipt verify response"), "ram-lfe receipt verify response")
        return RamLfeReceiptVerifyResponse(
            root["valid"] == true,
            requiredString(root["program_id"], "ram-lfe receipt verify response.program_id"),
            requiredString(root["backend"], "ram-lfe receipt verify response.backend"),
            normalizedMode(requiredString(root["verification_mode"], "ram-lfe receipt verify response.verification_mode")),
            requiredString(root["output_hash"], "ram-lfe receipt verify response.output_hash"),
            requiredString(root["associated_data_hash"], "ram-lfe receipt verify response.associated_data_hash"),
            if (root.containsKey("output_hash_matches"))
                asOptionalBoolean(root["output_hash_matches"], "ram-lfe receipt verify response.output_hash_matches")
            else null,
            optionalString(root["error"])
        )
    }

    private fun parse(payload: ByteArray?, context: String): Any? {
        check(payload != null && payload.isNotEmpty()) { "$context returned an empty payload" }
        val json = String(payload, StandardCharsets.UTF_8).trim()
        check(json.isNotEmpty()) { "$context returned a blank payload" }
        return JsonParser.parse(json)
    }

    @Suppress("UNCHECKED_CAST")
    private fun expectObject(value: Any?, path: String): Map<String, Any> {
        check(value is Map<*, *>) { "$path must be a JSON object" }
        return value as Map<String, Any>
    }

    @Suppress("UNCHECKED_CAST")
    private fun asArrayOrEmpty(value: Any?, path: String): List<Any?> {
        if (value == null) return emptyList()
        check(value is List<*>) { "$path must be a JSON array" }
        return value as List<Any?>
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

    private fun asLong(value: Any?, path: String): Long {
        check(value is Number) { "$path must be a number" }
        check(value !is Float && value !is Double) { "$path must be an integer" }
        return value.toLong()
    }

    private fun asOptionalLong(value: Any?, path: String): Long? {
        if (value == null) return null
        return asLong(value, path)
    }

    private fun asOptionalBoolean(value: Any?, path: String): Boolean? {
        if (value == null) return null
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun canonicalizeHex(value: String, context: String): String {
        var trimmed = value.trim()
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
            trimmed = trimmed.substring(2)
        }
        require(trimmed.length % 2 == 0 && trimmed.matches(Regex("(?i)[0-9a-f]+"))) {
            "$context must contain an even number of hex characters"
        }
        return trimmed.lowercase()
    }

    private fun normalizedMode(value: String): String = value.trim().lowercase()

    private fun parseBfvPublicParameters(root: Map<String, Any?>, context: String): IdentifierBfvPublicParameters {
        val parameters = expectObject(root["parameters"], "$context.parameters")
        val publicKey = expectObject(root["public_key"], "$context.public_key")
        return IdentifierBfvPublicParameters(
            IdentifierBfvPublicParameters.Parameters(
                asLong(parameters["polynomial_degree"], "$context.parameters.polynomial_degree"),
                asLong(parameters["plaintext_modulus"], "$context.parameters.plaintext_modulus"),
                asLong(parameters["ciphertext_modulus"], "$context.parameters.ciphertext_modulus"),
                Math.toIntExact(asLong(parameters["decomposition_base_log"], "$context.parameters.decomposition_base_log"))
            ),
            IdentifierBfvPublicParameters.PublicKey(
                asLongList(publicKey["b"], "$context.public_key.b"),
                asLongList(publicKey["a"], "$context.public_key.a")
            ),
            Math.toIntExact(asLong(root["max_input_bytes"], "$context.max_input_bytes"))
        )
    }

    private fun asLongList(value: Any?, path: String): List<Long> {
        val values = asArrayOrEmpty(value, path)
        return values.mapIndexed { index, v -> asLong(v, "$path[$index]") }
    }
}
