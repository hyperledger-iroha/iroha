package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import java.util.Base64

/** Minimal JSON parser for Torii contract deploy/call responses. */
object ContractJsonParser {

    @JvmStatic
    fun parseDeployResponse(payload: ByteArray): ContractDeployResponse {
        val root = expectObject(parse(payload, "contract deploy response"), "contract deploy response")
        val contracts = requiredList(root["contracts"], "contract deploy response.contracts")
        val initCalls = requiredList(root["init_calls"], "contract deploy response.init_calls")
        val assertions = requiredList(root["assertions"], "contract deploy response.assertions")
        return ContractDeployResponse(
            ok = root["ok"] == true,
            bundleName = requiredString(root["bundle_name"], "contract deploy response.bundle_name"),
            bundleDigest = requiredString(root["bundle_digest"], "contract deploy response.bundle_digest"),
            chainFingerprint = requiredString(root["chain_fingerprint"], "contract deploy response.chain_fingerprint"),
            dryRun = root["dry_run"] == true,
            completedStages = requiredStringList(root["completed_stages"], "contract deploy response.completed_stages"),
            failurePoint = optionalString(root["failure_point"]),
            contracts = contracts.mapIndexed { index, item ->
                val contract = expectObject(item, "contract deploy response.contracts[$index]")
                ContractDeployResponseContract(
                    name = requiredString(contract["name"], "contract deploy response.contracts[$index].name"),
                    contractAlias = optionalString(contract["contract_alias"]),
                    contractAddress = optionalString(contract["contract_address"]),
                    previousContractAddress = optionalString(contract["previous_contract_address"]),
                    upgraded = contract["upgraded"] == true,
                    dataspace = optionalString(contract["dataspace"]),
                    deployNonce = if (contract.containsKey("deploy_nonce")) {
                        asOptionalLong(contract["deploy_nonce"], "contract deploy response.contracts[$index].deploy_nonce")
                    } else null,
                    txHashHex = if (contract.containsKey("tx_hash_hex") && contract["tx_hash_hex"] != null) {
                        HttpClientTransport.normalizeHex32(
                            requiredString(contract["tx_hash_hex"], "contract deploy response.contracts[$index].tx_hash_hex"),
                            "txHashHex",
                        )
                    } else null,
                    codeHashHex = HttpClientTransport.normalizeHex32(
                        requiredString(contract["code_hash_hex"], "contract deploy response.contracts[$index].code_hash_hex"),
                        "codeHashHex",
                    ),
                    abiHashHex = HttpClientTransport.normalizeHex32(
                        requiredString(contract["abi_hash_hex"], "contract deploy response.contracts[$index].abi_hash_hex"),
                        "abiHashHex",
                    ),
                    status = requiredString(contract["status"], "contract deploy response.contracts[$index].status"),
                )
            },
            initCalls = initCalls.mapIndexed { index, item ->
                val call = expectObject(item, "contract deploy response.init_calls[$index]")
                ContractDeployResponseInitCall(
                    id = requiredString(call["id"], "contract deploy response.init_calls[$index].id"),
                    contractAlias = optionalString(call["contract_alias"]),
                    entrypoint = optionalString(call["entrypoint"]),
                    txHashHex = if (call.containsKey("tx_hash_hex") && call["tx_hash_hex"] != null) {
                        HttpClientTransport.normalizeHex32(
                            requiredString(call["tx_hash_hex"], "contract deploy response.init_calls[$index].tx_hash_hex"),
                            "txHashHex",
                        )
                    } else null,
                    status = requiredString(call["status"], "contract deploy response.init_calls[$index].status"),
                )
            },
            assertions = assertions.mapIndexed { index, item ->
                val assertion = expectObject(item, "contract deploy response.assertions[$index]")
                ContractDeployResponseAssertion(
                    id = requiredString(assertion["id"], "contract deploy response.assertions[$index].id"),
                    contractAlias = optionalString(assertion["contract_alias"]),
                    entrypoint = optionalString(assertion["entrypoint"]),
                    status = requiredString(assertion["status"], "contract deploy response.assertions[$index].status"),
                    actualResult = assertion["actual_result"],
                    expectedResult = assertion["expected_result"],
                    error = optionalString(assertion["error"]),
                )
            },
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
        return JsonNumbers.asLong(value, path)
    }

    private fun asOptionalLong(value: Any?, path: String): Long? {
        if (value == null) return null
        return asLong(value, path)
    }

    @Suppress("UNCHECKED_CAST")
    private fun requiredList(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be an array" }
        return value as List<Any?>
    }

    private fun requiredStringList(value: Any?, path: String): List<String> {
        return requiredList(value, path).mapIndexed { index, item ->
            requiredString(item, "$path[$index]")
        }
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
