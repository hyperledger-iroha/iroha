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
                    optionalString(item["note"]),
                    if (item["proof_verifier"] == null) null
                    else parseProofVerifier(
                        expectObject(item["proof_verifier"], "identifier policy list.items[$i].proof_verifier"),
                        "identifier policy list.items[$i].proof_verifier"
                    )
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
        val receiptPayload = parseResolutionPayload(
            expectObject(root["payload"], "identifier resolution receipt.payload"),
            "identifier resolution receipt.payload"
        )
        val attestation = parseReceiptAttestation(
            expectObject(root["attestation"], "identifier resolution receipt.attestation"),
            "identifier resolution receipt.attestation"
        )
        return IdentifierResolutionReceipt(
            receiptPayload,
            attestation
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
        return JsonNumbers.asLong(value, path)
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
                JsonNumbers.asInt(parameters["decomposition_base_log"], "$context.parameters.decomposition_base_log")
            ),
            IdentifierBfvPublicParameters.PublicKey(
                asLongList(publicKey["b"], "$context.public_key.b"),
                asLongList(publicKey["a"], "$context.public_key.a")
            ),
            JsonNumbers.asInt(root["max_input_bytes"], "$context.max_input_bytes")
        )
    }

    private fun parseProofVerifier(root: Map<String, Any?>, context: String): RamLfeProofVerifierMetadata =
        RamLfeProofVerifierMetadata(
            requiredString(root["proof_backend"], "$context.proof_backend"),
            requiredString(root["circuit_id"], "$context.circuit_id"),
            canonicalizeHex32(requiredString(root["public_inputs_schema_hash"], "$context.public_inputs_schema_hash"), "$context.public_inputs_schema_hash"),
            requiredString(root["verifying_key_bytes_b64"], "$context.verifying_key_bytes_b64")
        )

    private fun parseResolutionPayload(root: Map<String, Any?>, context: String): IdentifierResolutionPayload {
        val execution = parseResolutionExecutionPayload(
            expectObject(root["execution"], "$context.execution"),
            "$context.execution"
        )
        return IdentifierResolutionPayload(
            requiredString(root["policy_id"], "$context.policy_id"),
            execution,
            canonicalizeOpaque(requiredString(root["opaque_id"], "$context.opaque_id"), "$context.opaque_id"),
            canonicalizeHex32(requiredString(root["receipt_hash"], "$context.receipt_hash"), "$context.receipt_hash"),
            UaidLiteral.canonicalize(requiredString(root["uaid"], "$context.uaid"), "$context.uaid"),
            requiredString(root["account_id"], "$context.account_id")
        )
    }

    private fun parseResolutionExecutionPayload(root: Map<String, Any?>, context: String): IdentifierResolutionExecutionPayload =
        IdentifierResolutionExecutionPayload(
            requiredString(root["program_id"], "$context.program_id"),
            canonicalizeHex32(requiredString(root["program_digest"], "$context.program_digest"), "$context.program_digest"),
            requiredString(root["backend"], "$context.backend").lowercase(),
            requiredString(root["verification_mode"], "$context.verification_mode").lowercase(),
            canonicalizeHex32(requiredString(root["output_hash"], "$context.output_hash"), "$context.output_hash"),
            canonicalizeHex32(requiredString(root["associated_data_hash"], "$context.associated_data_hash"), "$context.associated_data_hash"),
            asLong(root["executed_at_ms"], "$context.executed_at_ms"),
            if (root.containsKey("expires_at_ms")) asOptionalLong(root["expires_at_ms"], "$context.expires_at_ms") else null
        )

    private fun parseReceiptAttestation(root: Map<String, Any?>, context: String): IdentifierReceiptAttestation {
        val kind = requiredString(root["kind"], "$context.kind").lowercase()
        return when (kind) {
            "signed" -> {
                val signature = canonicalizeHex(requiredString(root["signature"], "$context.signature"), "$context.signature")
                check(root["proof_backend"] == null && root["proof_b64"] == null) {
                    "$context signed attestation must not include proof fields"
                }
                IdentifierReceiptAttestation(kind, signature, null, null)
            }
            "proof" -> {
                val backend = requiredString(root["proof_backend"], "$context.proof_backend")
                val proofB64 = requiredString(root["proof_b64"], "$context.proof_b64")
                check(root["signature"] == null) {
                    "$context proof attestation must not include signature"
                }
                IdentifierReceiptAttestation(kind, null, backend, proofB64)
            }
            else -> error("$context.kind must be signed or proof")
        }
    }

    private fun asLongList(value: Any?, path: String): List<Long> {
        val values = asArrayOrEmpty(value, path)
        return values.mapIndexed { index, v -> asLong(v, "$path[$index]") }
    }
}
