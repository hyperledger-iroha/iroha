package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import java.util.Base64
import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.NetworkId

/** Minimal JSON parser for Soracloud private uploaded-model execute and receipt surfaces. */
object SoracloudPrivateUploadedModelJsonParser {

    private const val U32_MAX = 4_294_967_295L
    private const val SUBMITTED = "submitted"
    private const val COMMITTED = "committed"
    private const val X25519_HKDF_SHA256 = "X25519HkdfSha256"
    private const val AES_256_GCM = "Aes256Gcm"
    private val EXECUTE_RESPONSE_FIELDS = setOf(
        "schema_version", "status", "submission_status", "transaction_hash", "receipt",
        "output_artifact",
    )
    private val RECEIPT_FIELDS = setOf(
        "schema_version", "network_id", "receipt_id", "service_name", "service_version", "model_id",
        "weight_version", "runtime_version", "model_manifest_digest", "model_bundle_root",
        "policy_id", "decryption_request_id", "attesting_validator", "input_artifact",
        "output_artifact", "input_commitment", "output_commitment", "output_recipient",
        "request_commitment", "result_commitment", "emitted_sequence", "emitted_block_height",
    )
    private val ARTIFACT_FIELDS = setOf(
        "schema_version", "sorafs_manifest_digest", "sorafs_root_cid", "artifact_hash",
        "ciphertext_bytes", "artifact_role",
    )
    private val ATTESTING_VALIDATOR_FIELDS = setOf("lane_id", "validator_account_id", "peer_id")
    private val OUTPUT_RECIPIENT_FIELDS = setOf(
        "schema_version", "key_id", "key_version", "kem", "aead", "public_key_bytes",
        "public_key_fingerprint",
    )
    private val KEM_FIELDS = setOf("kem", "value")
    private val AEAD_FIELDS = setOf("aead", "value")
    private val RECEIPT_LIST_REQUIRED_FIELDS = setOf(
        "schema_version", "receipts", "returned_items", "remaining_items", "has_more",
        "count_mode",
    )
    private val RECEIPT_LIST_ALLOWED_FIELDS = RECEIPT_LIST_REQUIRED_FIELDS +
        setOf("total", "continue_cursor")

    @JvmStatic
    fun parseExecuteResponse(payload: ByteArray): SoracloudPrivateUploadedModelExecuteResponse {
        val root = expectObject(parse(payload, "soracloud private execute response"), "soracloud private execute response")
        requireFields(
            root,
            allowed = EXECUTE_RESPONSE_FIELDS,
            required = EXECUTE_RESPONSE_FIELDS,
            path = "soracloud private execute response",
        )
        val submissionStatus = submissionStatus(
            root["submission_status"],
            "soracloud private execute response.submission_status",
        )
        val transactionHash = optionalNonBlankString(
            root["transaction_hash"],
            "soracloud private execute response.transaction_hash",
        )
        requireTransactionHashMatchesStatus(submissionStatus, transactionHash)
        val receipt = parseReceipt(
            expectObject(root["receipt"], "soracloud private execute response.receipt"),
            "soracloud private execute response.receipt",
        )
        requireReceiptPersistenceMatchesStatus(submissionStatus, receipt)
        val outputArtifact = parseArtifact(
            expectObject(
                root["output_artifact"],
                "soracloud private execute response.output_artifact",
            ),
            "soracloud private execute response.output_artifact",
            requiredRole = "output",
        )
        check(outputArtifact == receipt.outputArtifact) {
            "soracloud private execute response.output_artifact must match receipt.output_artifact"
        }
        return SoracloudPrivateUploadedModelExecuteResponse(
            schemaVersion = schemaVersion(root["schema_version"], "soracloud private execute response.schema_version"),
            status = expectObject(root["status"], "soracloud private execute response.status"),
            submissionStatus = submissionStatus,
            transactionHash = transactionHash,
            receipt = receipt,
            outputArtifact = outputArtifact,
        )
    }

    @JvmStatic
    fun parseReceiptList(payload: ByteArray): SoracloudPrivateUploadedModelReceiptListResponse {
        val root = expectObject(parse(payload, "soracloud private receipt list"), "soracloud private receipt list")
        requireFields(
            root,
            allowed = RECEIPT_LIST_ALLOWED_FIELDS,
            required = RECEIPT_LIST_REQUIRED_FIELDS,
            path = "soracloud private receipt list",
        )
        val receiptValues = asArray(root["receipts"], "soracloud private receipt list.receipts")
        val receipts = ArrayList<SoracloudPrivateUploadedModelExecutionReceipt>(receiptValues.size)
        for (i in receiptValues.indices) {
            receipts.add(parseReceipt(expectObject(receiptValues[i], "soracloud private receipt list.receipts[$i]"), "soracloud private receipt list.receipts[$i]"))
        }
        return SoracloudPrivateUploadedModelReceiptListResponse(
            schemaVersion = schemaVersion(root["schema_version"], "soracloud private receipt list.schema_version"),
            receipts = receipts,
            total = if (root.containsKey("total")) asOptionalNonNegativeLong(root["total"], "soracloud private receipt list.total") else null,
            returnedItems = asNonNegativeLong(root["returned_items"], "soracloud private receipt list.returned_items"),
            remainingItems = asNonNegativeLong(root["remaining_items"], "soracloud private receipt list.remaining_items"),
            hasMore = asBoolean(root["has_more"], "soracloud private receipt list.has_more"),
            countMode = requiredString(root["count_mode"], "soracloud private receipt list.count_mode").lowercase(),
            continueCursor = optionalString(root["continue_cursor"], "soracloud private receipt list.continue_cursor"),
        )
    }

    private fun parseReceipt(
        root: Map<String, Any?>,
        context: String,
    ): SoracloudPrivateUploadedModelExecutionReceipt {
        requireFields(root, allowed = RECEIPT_FIELDS, required = RECEIPT_FIELDS, path = context)
        return SoracloudPrivateUploadedModelExecutionReceipt(
            schemaVersion = schemaVersion(root["schema_version"], "$context.schema_version"),
            networkId = networkId(root["network_id"], "$context.network_id"),
            receiptId = requiredString(root["receipt_id"], "$context.receipt_id"),
            serviceName = requiredString(root["service_name"], "$context.service_name"),
            serviceVersion = requiredString(root["service_version"], "$context.service_version"),
            modelId = requiredString(root["model_id"], "$context.model_id"),
            weightVersion = requiredString(root["weight_version"], "$context.weight_version"),
            runtimeVersion = requiredString(root["runtime_version"], "$context.runtime_version"),
            modelManifestDigest = requiredString(root["model_manifest_digest"], "$context.model_manifest_digest"),
            modelBundleRoot = requiredString(root["model_bundle_root"], "$context.model_bundle_root"),
            policyId = requiredString(root["policy_id"], "$context.policy_id"),
            decryptionRequestId = requiredString(root["decryption_request_id"], "$context.decryption_request_id"),
            attestingValidator = parseAttestingValidator(
                expectObject(root["attesting_validator"], "$context.attesting_validator"),
                "$context.attesting_validator",
            ),
            inputArtifact = parseArtifact(
                expectObject(root["input_artifact"], "$context.input_artifact"),
                "$context.input_artifact",
                requiredRole = "input",
            ),
            outputArtifact = parseArtifact(
                expectObject(root["output_artifact"], "$context.output_artifact"),
                "$context.output_artifact",
                requiredRole = "output",
            ),
            inputCommitment = requiredString(root["input_commitment"], "$context.input_commitment"),
            outputCommitment = requiredString(root["output_commitment"], "$context.output_commitment"),
            outputRecipient = parseOutputRecipient(
                expectObject(root["output_recipient"], "$context.output_recipient"),
                "$context.output_recipient",
            ),
            requestCommitment = requiredString(root["request_commitment"], "$context.request_commitment"),
            resultCommitment = requiredString(root["result_commitment"], "$context.result_commitment"),
            emittedSequence = asNonNegativeLong(root["emitted_sequence"], "$context.emitted_sequence"),
            emittedBlockHeight = asNonNegativeLong(
                root["emitted_block_height"],
                "$context.emitted_block_height",
            ),
        )
    }

    private fun parseArtifact(
        root: Map<String, Any?>,
        context: String,
        requiredRole: String,
    ): SoracloudPrivateModelArtifactRef {
        requireFields(root, allowed = ARTIFACT_FIELDS, required = ARTIFACT_FIELDS, path = context)
        val artifactRole = requiredString(root["artifact_role"], "$context.artifact_role")
        check(artifactRole == requiredRole) {
            "$context.artifact_role must equal `$requiredRole`"
        }
        return SoracloudPrivateModelArtifactRef(
            schemaVersion = schemaVersion(root["schema_version"], "$context.schema_version"),
            sorafsManifestDigest = requiredString(root["sorafs_manifest_digest"], "$context.sorafs_manifest_digest"),
            sorafsRootCid = sorafsRootCid(root["sorafs_root_cid"], "$context.sorafs_root_cid"),
            artifactHash = requiredString(root["artifact_hash"], "$context.artifact_hash"),
            ciphertextBytes = asPositiveLong(root["ciphertext_bytes"], "$context.ciphertext_bytes"),
            artifactRole = artifactRole,
        )
    }

    private fun parseAttestingValidator(
        root: Map<String, Any?>,
        context: String,
    ): SoracloudRuntimeDeterministicValidatorHost {
        requireFields(
            root,
            allowed = ATTESTING_VALIDATOR_FIELDS,
            required = ATTESTING_VALIDATOR_FIELDS,
            path = context,
        )
        return SoracloudRuntimeDeterministicValidatorHost(
            laneId = boundedLong(root["lane_id"], "$context.lane_id", 0L, U32_MAX),
            validatorAccountId = requiredString(root["validator_account_id"], "$context.validator_account_id"),
            peerId = requiredString(root["peer_id"], "$context.peer_id"),
        )
    }

    private fun parseOutputRecipient(
        root: Map<String, Any?>,
        context: String,
    ): SoracloudUploadedModelEncryptionRecipient {
        requireFields(
            root,
            allowed = OUTPUT_RECIPIENT_FIELDS,
            required = OUTPUT_RECIPIENT_FIELDS,
            path = context,
        )
        return SoracloudUploadedModelEncryptionRecipient(
            schemaVersion = schemaVersion(root["schema_version"], "$context.schema_version"),
            keyId = requiredString(root["key_id"], "$context.key_id"),
            keyVersion = boundedLong(root["key_version"], "$context.key_version", 1L, U32_MAX),
            kem = parseUnitSuite(
                expectObject(root["kem"], "$context.kem"),
                fields = KEM_FIELDS,
                tag = "kem",
                expected = X25519_HKDF_SHA256,
                context = "$context.kem",
            ),
            aead = parseUnitSuite(
                expectObject(root["aead"], "$context.aead"),
                fields = AEAD_FIELDS,
                tag = "aead",
                expected = AES_256_GCM,
                context = "$context.aead",
            ),
            publicKeyBytesBase64 = canonicalBase64X25519Key(
                root["public_key_bytes"],
                "$context.public_key_bytes",
            ),
            publicKeyFingerprint = requiredString(
                root["public_key_fingerprint"],
                "$context.public_key_fingerprint",
            ),
        )
    }

    private fun parseUnitSuite(
        root: Map<String, Any?>,
        fields: Set<String>,
        tag: String,
        expected: String,
        context: String,
    ): String {
        requireFields(root, allowed = fields, required = fields, path = context)
        val actual = requiredString(root[tag], "$context.$tag")
        check(actual == expected) { "$context.$tag must equal `$expected`" }
        check(root["value"] == null) { "$context.value must be null" }
        return actual
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

    private fun asArray(value: Any?, path: String): List<Any?> {
        check(value is List<*>) { "$path must be a JSON array" }
        return value
    }

    private fun requiredString(value: Any?, path: String): String {
        val string = optionalString(value, path)
        check(!string.isNullOrBlank()) { "$path must be a non-empty string" }
        return string.trim()
    }

    private fun optionalString(value: Any?, path: String): String? {
        if (value == null) return null
        check(value is String) { "$path must be a string" }
        return value
    }

    private fun optionalNonBlankString(value: Any?, path: String): String? =
        if (value == null) null else requiredString(value, path)

    private fun asLong(value: Any?, path: String): Long = JsonNumbers.asLong(value, path)

    private fun asNonNegativeLong(value: Any?, path: String): Long {
        val parsed = asLong(value, path)
        check(parsed >= 0) { "$path must be non-negative" }
        return parsed
    }

    private fun asPositiveLong(value: Any?, path: String): Long {
        val parsed = asLong(value, path)
        check(parsed > 0) { "$path must be greater than zero" }
        return parsed
    }

    private fun boundedLong(value: Any?, path: String, minimum: Long, maximum: Long): Long {
        val parsed = asLong(value, path)
        check(parsed in minimum..maximum) { "$path must be within $minimum..=$maximum" }
        return parsed
    }

    private fun asOptionalNonNegativeLong(value: Any?, path: String): Long? =
        if (value == null) null else asNonNegativeLong(value, path)

    private fun asBoolean(value: Any?, path: String): Boolean {
        check(value is Boolean) { "$path must be a boolean" }
        return value
    }

    private fun schemaVersion(value: Any?, path: String): Long {
        val parsed = asLong(value, path)
        check(parsed == 1L) { "$path must equal 1" }
        return parsed
    }

    private fun networkId(value: Any?, path: String): String {
        val literal = requiredString(value, path)
        return try {
            NetworkId.parse(literal).literal
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(
                "$path must be an exact canonical checksummed 32-byte NetworkId literal",
                error,
            )
        }
    }

    private fun sorafsRootCid(value: Any?, path: String): List<Int> {
        val values = asArray(value, path)
        check(values.size == 36) { "$path must contain exactly 36 unsigned integer bytes" }
        val bytes = values.mapIndexed { index, element ->
            boundedLong(element, "$path[$index]", 0L, 255L).toInt()
        }
        check(bytes.subList(0, 4) == listOf(1, 0x71, 0x1f, 32)) {
            "$path must use canonical CIDv1/dag-cbor/BLAKE3-256 framing"
        }
        check(bytes.subList(4, 36).any { it != 0 }) { "$path digest must be nonzero" }
        return Collections.unmodifiableList(bytes)
    }

    private fun submissionStatus(value: Any?, path: String): String {
        val parsed = requiredString(value, path)
        check(parsed == SUBMITTED || parsed == COMMITTED) {
            "$path must equal `submitted` or `committed`"
        }
        return parsed
    }

    private fun requireTransactionHashMatchesStatus(
        submissionStatus: String,
        transactionHash: String?,
    ) {
        check(submissionStatus != SUBMITTED || transactionHash != null) {
            "soracloud private execute response.transaction_hash is required for `submitted`"
        }
        check(submissionStatus != COMMITTED || transactionHash == null) {
            "soracloud private execute response.transaction_hash must be null for `committed`"
        }
    }

    private fun requireReceiptPersistenceMatchesStatus(
        submissionStatus: String,
        receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    ) {
        check(
            submissionStatus != SUBMITTED ||
                (receipt.emittedSequence == 0L && receipt.emittedBlockHeight == 0L)
        ) {
            "soracloud private execute response.receipt must use zero ledger coordinates for `submitted`"
        }
        check(
            submissionStatus != COMMITTED ||
                (receipt.emittedSequence > 0L && receipt.emittedBlockHeight > 0L)
        ) {
            "soracloud private execute response.receipt must use positive ledger coordinates for `committed`"
        }
    }

    private fun canonicalBase64X25519Key(value: Any?, path: String): String {
        val encoded = requiredString(value, path)
        val decoded = try {
            Base64.getDecoder().decode(encoded)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$path must be canonical base64", error)
        }
        check(decoded.size == 32 && Base64.getEncoder().encodeToString(decoded) == encoded) {
            "$path must be canonical base64 encoding exactly 32 bytes"
        }
        return encoded
    }

    private fun requireFields(
        root: Map<String, Any?>,
        allowed: Set<String>,
        required: Set<String>,
        path: String,
    ) {
        val unknown = root.keys.firstOrNull { it !in allowed }
        check(unknown == null) { "$path contains unknown field `$unknown`" }
        val missing = required.firstOrNull { !root.containsKey(it) }
        check(missing == null) { "$path.$missing is missing required field" }
    }
}
