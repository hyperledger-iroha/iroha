package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.nexus.UaidLiteral

/** Minimal JSON parser for identifier-policy and identifier-resolution payloads. */
object IdentifierJsonParser {

    @JvmStatic
    fun parsePolicyList(payload: ByteArray): IdentifierPolicyListResponse {
        val root = expectObject(parse(payload, "identifier policy list"), "identifier policy list")
        val itemValues = asArrayOrEmpty(root["items"], "identifier policy list.items")
        val items = ArrayList<IdentifierPolicySummary>(itemValues.size)
        for (i in itemValues.indices) {
            val item = expectObject(itemValues[i], "identifier policy list.items[$i]")
            items.add(
                IdentifierPolicySummary(
                    requiredString(item["policy_id"], "identifier policy list.items[$i].policy_id"),
                    requiredString(item["owner"], "identifier policy list.items[$i].owner"),
                    item["active"] == true,
                    IdentifierNormalization.fromWireValue(
                        requiredString(item["normalization"], "identifier policy list.items[$i].normalization")
                    ),
                    requiredString(item["resolver_public_key"], "identifier policy list.items[$i].resolver_public_key"),
                    requiredString(item["backend"], "identifier policy list.items[$i].backend"),
                    optionalString(item["input_encryption"]),
                    optionalString(item["input_encryption_public_parameters"]),
                    if (item["input_encryption_public_parameters_decoded"] == null) null
                    else parseBfvPublicParameters(
                        expectObject(item["input_encryption_public_parameters_decoded"],
                            "identifier policy list.items[$i].input_encryption_public_parameters_decoded"),
                        "identifier policy list.items[$i].input_encryption_public_parameters_decoded"
                    ),
                    optionalString(item["note"])
                )
            )
        }
        val total = if (root.containsKey("total"))
            asLong(root["total"], "identifier policy list.total")
        else items.size.toLong()
        return IdentifierPolicyListResponse(total, items)
    }

    @JvmStatic
    fun parseResolutionReceipt(payload: ByteArray): IdentifierResolutionReceipt {
        val root = expectObject(parse(payload, "identifier resolution receipt"), "identifier resolution receipt")
        return IdentifierResolutionReceipt(
            requiredString(root["policy_id"], "identifier resolution receipt.policy_id"),
            canonicalizeOpaque(requiredString(root["opaque_id"], "identifier resolution receipt.opaque_id"), "identifier resolution receipt.opaque_id"),
            canonicalizeHex32(requiredString(root["receipt_hash"], "identifier resolution receipt.receipt_hash"), "identifier resolution receipt.receipt_hash"),
            UaidLiteral.canonicalize(requiredString(root["uaid"], "identifier resolution receipt.uaid"), "identifier resolution receipt.uaid"),
            requiredString(root["account_id"], "identifier resolution receipt.account_id"),
            asLong(root["resolved_at_ms"], "identifier resolution receipt.resolved_at_ms"),
            if (root.containsKey("expires_at_ms")) asOptionalLong(root["expires_at_ms"], "identifier resolution receipt.expires_at_ms") else null,
            requiredString(root["backend"], "identifier resolution receipt.backend"),
            requiredString(root["signature"], "identifier resolution receipt.signature"),
            canonicalizeHex(requiredString(root["signature_payload_hex"], "identifier resolution receipt.signature_payload_hex"), "identifier resolution receipt.signature_payload_hex"),
            parseResolutionPayload(
                expectObject(root["signature_payload"], "identifier resolution receipt.signature_payload"),
                "identifier resolution receipt.signature_payload"
            )
        )
    }

    @JvmStatic
    fun parseClaimRecord(payload: ByteArray): IdentifierClaimRecord {
        val root = expectObject(parse(payload, "identifier claim record"), "identifier claim record")
        return IdentifierClaimRecord(
            requiredString(root["policy_id"], "identifier claim record.policy_id"),
            canonicalizeOpaque(requiredString(root["opaque_id"], "identifier claim record.opaque_id"), "identifier claim record.opaque_id"),
            canonicalizeHex32(requiredString(root["receipt_hash"], "identifier claim record.receipt_hash"), "identifier claim record.receipt_hash"),
            UaidLiteral.canonicalize(requiredString(root["uaid"], "identifier claim record.uaid"), "identifier claim record.uaid"),
            requiredString(root["account_id"], "identifier claim record.account_id"),
            asLong(root["verified_at_ms"], "identifier claim record.verified_at_ms"),
            if (root.containsKey("expires_at_ms")) asOptionalLong(root["expires_at_ms"], "identifier claim record.expires_at_ms") else null
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

    private fun canonicalizeOpaque(value: String, context: String): String {
        val literal = value.trim()
        require(literal.isNotEmpty()) { "$context must not be blank" }
        val hexPortion = if (literal.lowercase().startsWith("opaque:")) literal.substring("opaque:".length) else literal
        val trimmedHex = hexPortion.trim()
        require(trimmedHex.length == 64 && trimmedHex.matches(Regex("(?i)[0-9a-f]{64}"))) {
            "$context must contain 64 hex characters"
        }
        return "opaque:${trimmedHex.lowercase()}"
    }

    private fun canonicalizeHex32(value: String, context: String): String {
        var trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$context must not be blank" }
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
            trimmed = trimmed.substring(2)
        }
        require(trimmed.length == 64 && trimmed.matches(Regex("(?i)[0-9a-f]{64}"))) {
            "$context must contain 64 hex characters"
        }
        return trimmed.lowercase()
    }

    private fun canonicalizeHex(value: String, context: String): String {
        var trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$context must not be blank" }
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
            trimmed = trimmed.substring(2)
        }
        require(trimmed.length % 2 == 0 && trimmed.matches(Regex("(?i)[0-9a-f]+"))) {
            "$context must contain an even number of hex characters"
        }
        return trimmed.lowercase()
    }

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

    private fun parseResolutionPayload(root: Map<String, Any?>, context: String): IdentifierResolutionPayload {
        return IdentifierResolutionPayload(
            requiredString(root["policy_id"], "$context.policy_id"),
            canonicalizeOpaque(requiredString(root["opaque_id"], "$context.opaque_id"), "$context.opaque_id"),
            canonicalizeHex32(requiredString(root["receipt_hash"], "$context.receipt_hash"), "$context.receipt_hash"),
            UaidLiteral.canonicalize(requiredString(root["uaid"], "$context.uaid"), "$context.uaid"),
            requiredString(root["account_id"], "$context.account_id"),
            asLong(root["resolved_at_ms"], "$context.resolved_at_ms"),
            if (root.containsKey("expires_at_ms")) asOptionalLong(root["expires_at_ms"], "$context.expires_at_ms") else null
        )
    }

    private fun asLongList(value: Any?, path: String): List<Long> {
        val values = asArrayOrEmpty(value, path)
        return values.mapIndexed { index, v -> asLong(v, "$path[$index]") }
    }
}
