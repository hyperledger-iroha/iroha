package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Minimal JSON parser for Soracloud private uploaded-model execute and receipt surfaces. */
object SoracloudPrivateUploadedModelJsonParser {

    const val PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID: String =
        "iroha_data_model::isi::soracloud::RecordSoracloudPrivateUploadedModelExecutionReceipt"

    @JvmStatic
    fun parseExecuteResponse(payload: ByteArray): SoracloudPrivateUploadedModelExecuteResponse {
        val root = expectObject(parse(payload, "soracloud private execute response"), "soracloud private execute response")
        return SoracloudPrivateUploadedModelExecuteResponse(
            schemaVersion = asLong(root["schema_version"], "soracloud private execute response.schema_version"),
            status = expectObject(root["status"], "soracloud private execute response.status"),
            receipt = parseReceipt(expectObject(root["receipt"], "soracloud private execute response.receipt"), "soracloud private execute response.receipt"),
            txInstructions = parseTxInstructions(root["tx_instructions"], "soracloud private execute response.tx_instructions"),
        )
    }

    @JvmStatic
    fun parseReceiptList(payload: ByteArray): SoracloudPrivateUploadedModelReceiptListResponse {
        val root = expectObject(parse(payload, "soracloud private receipt list"), "soracloud private receipt list")
        val receiptValues = asArrayOrEmpty(root["receipts"], "soracloud private receipt list.receipts")
        val receipts = ArrayList<SoracloudPrivateUploadedModelExecutionReceipt>(receiptValues.size)
        for (i in receiptValues.indices) {
            receipts.add(parseReceipt(expectObject(receiptValues[i], "soracloud private receipt list.receipts[$i]"), "soracloud private receipt list.receipts[$i]"))
        }
        return SoracloudPrivateUploadedModelReceiptListResponse(
            schemaVersion = asLong(root["schema_version"], "soracloud private receipt list.schema_version"),
            receipts = receipts,
            total = if (root.containsKey("total")) asOptionalNonNegativeLong(root["total"], "soracloud private receipt list.total") else null,
            returnedItems = asNonNegativeLong(root["returned_items"], "soracloud private receipt list.returned_items"),
            remainingItems = asNonNegativeLong(root["remaining_items"], "soracloud private receipt list.remaining_items"),
            hasMore = asBoolean(root["has_more"], "soracloud private receipt list.has_more"),
            countMode = requiredString(root["count_mode"], "soracloud private receipt list.count_mode").lowercase(),
            continueCursor = optionalString(root["continue_cursor"]),
        )
    }

    @JvmStatic
    fun privateUploadedModelReceiptInstruction(txInstructions: List<SoracloudTxInstruction>): SoracloudTxInstruction {
        for (instruction in txInstructions) {
            if (instruction.wireId == PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID) {
                canonicalizeHex(instruction.payloadHex, "soracloud private receipt instruction.payload_hex")
                return instruction
            }
        }
        throw IllegalStateException("missing $PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID instruction skeleton")
    }

    private fun parseReceipt(root: Map<String, Any?>, context: String): SoracloudPrivateUploadedModelExecutionReceipt =
        SoracloudPrivateUploadedModelExecutionReceipt(
            schemaVersion = asLong(root["schema_version"], "$context.schema_version"),
            receiptId = requiredString(root["receipt_id"], "$context.receipt_id"),
            serviceName = requiredString(root["service_name"], "$context.service_name"),
            modelId = requiredString(root["model_id"], "$context.model_id"),
            weightVersion = requiredString(root["weight_version"], "$context.weight_version"),
            runtimeVersion = requiredString(root["runtime_version"], "$context.runtime_version"),
            modelManifestDigest = requiredString(root["model_manifest_digest"], "$context.model_manifest_digest"),
            modelBundleRoot = requiredString(root["model_bundle_root"], "$context.model_bundle_root"),
            policyId = requiredString(root["policy_id"], "$context.policy_id"),
            inputArtifact = parseArtifact(expectObject(root["input_artifact"], "$context.input_artifact"), "$context.input_artifact"),
            outputArtifact = parseArtifact(expectObject(root["output_artifact"], "$context.output_artifact"), "$context.output_artifact"),
            inputCommitment = requiredString(root["input_commitment"], "$context.input_commitment"),
            outputCommitment = requiredString(root["output_commitment"], "$context.output_commitment"),
            requestCommitment = requiredString(root["request_commitment"], "$context.request_commitment"),
            resultCommitment = requiredString(root["result_commitment"], "$context.result_commitment"),
            emittedSequence = asNonNegativeLong(root["emitted_sequence"], "$context.emitted_sequence"),
        )

    private fun parseArtifact(root: Map<String, Any?>, context: String): SoracloudPrivateModelArtifactRef =
        SoracloudPrivateModelArtifactRef(
            schemaVersion = asLong(root["schema_version"], "$context.schema_version"),
            sorafsManifestDigest = requiredString(root["sorafs_manifest_digest"], "$context.sorafs_manifest_digest"),
            artifactHash = requiredString(root["artifact_hash"], "$context.artifact_hash"),
            ciphertextBytes = asNonNegativeLong(root["ciphertext_bytes"], "$context.ciphertext_bytes"),
            artifactRole = requiredString(root["artifact_role"], "$context.artifact_role"),
        )

    private fun parseTxInstructions(value: Any?, path: String): List<SoracloudTxInstruction> {
        val values = asArrayOrEmpty(value, path)
        val instructions = ArrayList<SoracloudTxInstruction>(values.size)
        for (i in values.indices) {
            val root = expectObject(values[i], "$path[$i]")
            val payloadHex = requiredString(root["payload_hex"], "$path[$i].payload_hex")
            canonicalizeHex(payloadHex, "$path[$i].payload_hex")
            instructions.add(
                SoracloudTxInstruction(
                    wireId = requiredString(root["wire_id"], "$path[$i].wire_id"),
                    payloadHex = payloadHex,
                )
            )
        }
        return instructions
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

    private fun asArrayOrEmpty(value: Any?, path: String): List<Any?> {
        if (value == null) return emptyList()
        check(value is List<*>) { "$path must be a JSON array" }
        return value
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

    private fun asLong(value: Any?, path: String): Long = JsonNumbers.asLong(value, path)

    private fun asNonNegativeLong(value: Any?, path: String): Long {
        val parsed = asLong(value, path)
        check(parsed >= 0) { "$path must be non-negative" }
        return parsed
    }

    private fun asOptionalNonNegativeLong(value: Any?, path: String): Long? =
        if (value == null) null else asNonNegativeLong(value, path)

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun canonicalizeHex(value: String, context: String): String {
        var trimmed = value.trim()
        if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
            trimmed = trimmed.substring(2)
        }
        require(trimmed.isNotEmpty() && trimmed.length % 2 == 0 && trimmed.matches(Regex("(?i)[0-9a-f]+"))) {
            "$context must contain a non-empty even number of hex characters"
        }
        return trimmed.lowercase()
    }
}
