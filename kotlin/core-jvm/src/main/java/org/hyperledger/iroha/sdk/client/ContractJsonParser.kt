package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import java.util.Base64

/** Minimal JSON parser for Torii contract deploy/call responses. */
object ContractJsonParser {

    @JvmStatic
    fun parseDeployResponse(payload: ByteArray): ContractDeployResponse {
        val root = expectObject(parse(payload, "contract deploy response"), "contract deploy response")
        return ContractDeployResponse(
            ok = root["ok"] == true,
            contractAlias = optionalString(root["contract_alias"]),
            contractAddress = optionalString(root["contract_address"]),
            previousContractAddress = optionalString(root["previous_contract_address"]),
            upgraded = root["upgraded"] == true,
            dataspace = optionalString(root["dataspace"]),
            deployNonce = if (root.containsKey("deploy_nonce")) asOptionalLong(root["deploy_nonce"], "contract deploy response.deploy_nonce") else null,
            txHashHex = if (root.containsKey("tx_hash_hex") && root["tx_hash_hex"] != null)
                HttpClientTransport.normalizeHex32(requiredString(root["tx_hash_hex"], "contract deploy response.tx_hash_hex"), "txHashHex")
            else null,
            codeHashHex = HttpClientTransport.normalizeHex32(requiredString(root["code_hash_hex"], "contract deploy response.code_hash_hex"), "codeHashHex"),
            abiHashHex = HttpClientTransport.normalizeHex32(requiredString(root["abi_hash_hex"], "contract deploy response.abi_hash_hex"), "abiHashHex"),
        )
    }

    @JvmStatic
    fun parseCallResponse(payload: ByteArray): ContractCallResponse {
        val root = expectObject(parse(payload, "contract call response"), "contract call response")
        return ContractCallResponse(
            ok = root["ok"] == true,
            submitted = root["submitted"] == true,
            dataspace = requiredString(root["dataspace"], "contract call response.dataspace"),
            codeHashHex = HttpClientTransport.normalizeHex32(requiredString(root["code_hash_hex"], "contract call response.code_hash_hex"), "codeHashHex"),
            abiHashHex = HttpClientTransport.normalizeHex32(requiredString(root["abi_hash_hex"], "contract call response.abi_hash_hex"), "abiHashHex"),
            creationTimeMs = asLong(root["creation_time_ms"], "contract call response.creation_time_ms"),
            contractAddress = optionalString(root["contract_address"]),
            txHashHex = if (root.containsKey("tx_hash_hex") && root["tx_hash_hex"] != null)
                HttpClientTransport.normalizeHex32(requiredString(root["tx_hash_hex"], "contract call response.tx_hash_hex"), "txHashHex")
            else null,
            entrypoint = optionalString(root["entrypoint"]),
            transactionScaffoldB64 = optionalBase64(root["transaction_scaffold_b64"], "contract call response.transaction_scaffold_b64"),
            signedTransactionB64 = optionalBase64(root["signed_transaction_b64"], "contract call response.signed_transaction_b64"),
            signingMessageB64 = optionalBase64(root["signing_message_b64"], "contract call response.signing_message_b64"),
        )
    }

    @JvmStatic
    fun parseGovernanceContractResponse(payload: ByteArray): GovernanceContractResponse {
        val root = expectObject(parse(payload, "governance contract response"), "governance contract response")
        return GovernanceContractResponse(
            found = root["found"] == true,
            contractAddress = requiredString(root["contract_address"], "governance contract response.contract_address"),
            dataspace = optionalString(root["dataspace"]),
            codeHashHex = if (root.containsKey("code_hash_hex") && root["code_hash_hex"] != null)
                HttpClientTransport.normalizeHex32(requiredString(root["code_hash_hex"], "governance contract response.code_hash_hex"), "codeHashHex")
            else null,
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
        return if (value is String) value.trim().ifEmpty { null } else value.toString()
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

    private fun optionalBase64(value: Any?, path: String): String? {
        val literal = optionalString(value) ?: return null
        val decoded = try {
            Base64.getDecoder().decode(literal)
        } catch (ex: IllegalArgumentException) {
            throw IllegalStateException("$path must be valid base64", ex)
        }
        check(decoded.isNotEmpty()) { "$path must not decode to empty bytes" }
        return literal
    }
}
