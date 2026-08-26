package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.crypto.IrohaHash

/** Minimal JSON parser for Soracloud private uploaded-model execute and receipt surfaces. */
object SoracloudPrivateUploadedModelJsonParser {

    private const val U32_MAX = 4_294_967_295L
    private val U64_MAX = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
    private const val X25519_HKDF_SHA256 = "X25519HkdfSha256"
    private const val AES_256_GCM = "Aes256Gcm"
    private val EXECUTE_RESPONSE_FIELDS = setOf(
        "schema_version", "status", "submission_phase", "transaction_hash", "receipt",
        "output_artifact",
    )
    private val RECEIPT_FIELDS = setOf(
        "schema_version", "network_id", "receipt_id", "service_name", "service_version", "model_id",
        "weight_version", "runtime_version", "model_manifest_digest", "model_bundle_root",
        "policy_id", "decryption_request_id", "attesting_validator", "input_artifact",
        "output_artifact", "output_replication_order_id", "input_commitment", "output_commitment",
        "output_recipient", "request_commitment", "result_commitment",
        "authorization_claim_block_height", "authorization_claim_epoch", "emitted_sequence",
        "emitted_block_height", "emitted_epoch",
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
        "schema_version", "receipts", "total", "returned_items", "remaining_items", "has_more",
        "count_mode", "continue_cursor",
    )
    private val RECEIPT_LIST_ALLOWED_FIELDS = RECEIPT_LIST_REQUIRED_FIELDS

    @JvmStatic
    fun parseExecuteResponse(payload: ByteArray): SoracloudPrivateUploadedModelExecuteResponse {
        val root = expectObject(parse(payload, "soracloud private execute response"), "soracloud private execute response")
        requireFields(
            root,
            allowed = EXECUTE_RESPONSE_FIELDS,
            required = EXECUTE_RESPONSE_FIELDS,
            path = "soracloud private execute response",
        )
        val submissionPhase = submissionPhase(
            root["submission_phase"],
            "soracloud private execute response.submission_phase",
        )
        val transactionHash = optionalHashLiteral(
            root["transaction_hash"],
            "soracloud private execute response.transaction_hash",
        )
        requireTransactionHashMatchesPhase(submissionPhase, transactionHash)
        val receipt = parseReceipt(
            expectObject(root["receipt"], "soracloud private execute response.receipt"),
            "soracloud private execute response.receipt",
        )
        requireReceiptPersistenceMatchesPhase(submissionPhase, receipt)
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
        val status = parseUploadedModelStatus(
            expectObject(root["status"], "soracloud private execute response.status"),
            "soracloud private execute response.status",
        )
        try {
            requireUploadedModelStatusMatchesReceipt(
                status,
                receipt,
                "soracloud private execute response.status",
            )
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(error.message, error)
        }
        return SoracloudPrivateUploadedModelExecuteResponse(
            schemaVersion = schemaVersion(root["schema_version"], "soracloud private execute response.schema_version"),
            status = status,
            submissionPhase = submissionPhase,
            transactionHash = transactionHash,
            receipt = receipt,
            outputArtifact = outputArtifact,
        )
    }

    private fun parseUploadedModelStatus(
        root: Map<String, Any?>,
        context: String,
    ): Map<String, Any?> {
        try {
            SoracloudImmutableJsonObject.validateUploadedModelStatus(root, context)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(error.message, error)
        }
        return root
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
            val path = "soracloud private receipt list.receipts[$i]"
            val receipt = parseReceipt(expectObject(receiptValues[i], path), path)
            check(
                receipt.authorizationClaimBlockHeight > BigInteger.ZERO &&
                    receipt.authorizationClaimEpoch > BigInteger.ZERO &&
                    receipt.emittedSequence > BigInteger.ZERO &&
                    receipt.emittedBlockHeight > BigInteger.ZERO &&
                    receipt.emittedEpoch > BigInteger.ZERO
            ) {
                "$path must have positive ledger coordinates"
            }
            receipts.add(receipt)
        }
        val countMode = countMode(
            root["count_mode"],
            "soracloud private receipt list.count_mode",
        )
        val total = asOptionalBoundedLong(
            root["total"],
            "soracloud private receipt list.total",
            0L,
            U32_MAX,
        )
        check((countMode == "bounded") == (total == null)) {
            "soracloud private receipt list.total must be null for `bounded` and non-null for `exact`"
        }
        val returnedItems = boundedLong(
            root["returned_items"],
            "soracloud private receipt list.returned_items",
            0L,
            U32_MAX,
        )
        val remainingItems = asOptionalBoundedLong(
            root["remaining_items"],
            "soracloud private receipt list.remaining_items",
            0L,
            U32_MAX,
        )
        val hasMore = asBoolean(root["has_more"], "soracloud private receipt list.has_more")
        check(returnedItems == receipts.size.toLong()) {
            "soracloud private receipt list.returned_items must equal receipts.size"
        }
        check(receipts.zipWithNext().all { (left, right) ->
            left.emittedSequence < right.emittedSequence ||
                (left.emittedSequence == right.emittedSequence && left.receiptId < right.receiptId)
        }) {
            "soracloud private receipt list.receipts must use canonical ledger order"
        }
        val continueCursor = optionalNonBlankString(
            root["continue_cursor"],
            "soracloud private receipt list.continue_cursor",
        )
        return SoracloudPrivateUploadedModelReceiptListResponse(
            schemaVersion = schemaVersion(root["schema_version"], "soracloud private receipt list.schema_version"),
            receipts = receipts,
            total = total,
            returnedItems = returnedItems,
            remainingItems = remainingItems,
            hasMore = hasMore,
            countMode = countMode,
            continueCursor = continueCursor,
        )
    }

    private fun parseReceipt(
        root: Map<String, Any?>,
        context: String,
    ): SoracloudPrivateUploadedModelExecutionReceipt {
        requireFields(root, allowed = RECEIPT_FIELDS, required = RECEIPT_FIELDS, path = context)
        val emittedSequence = unsigned64Integer(
            root["emitted_sequence"],
            "$context.emitted_sequence",
        )
        val emittedBlockHeight = unsigned64Integer(
            root["emitted_block_height"],
            "$context.emitted_block_height",
        )
        val emittedEpoch = unsigned64Integer(
            root["emitted_epoch"],
            "$context.emitted_epoch",
        )
        val authorizationClaimBlockHeight = unsigned64Integer(
            root["authorization_claim_block_height"],
            "$context.authorization_claim_block_height",
        )
        val authorizationClaimEpoch = unsigned64Integer(
            root["authorization_claim_epoch"],
            "$context.authorization_claim_epoch",
        )
        check(
            (authorizationClaimBlockHeight == BigInteger.ZERO &&
                authorizationClaimEpoch == BigInteger.ZERO &&
                emittedSequence == BigInteger.ZERO &&
                emittedBlockHeight == BigInteger.ZERO &&
                emittedEpoch == BigInteger.ZERO) ||
                (authorizationClaimBlockHeight > BigInteger.ZERO &&
                    authorizationClaimEpoch > BigInteger.ZERO &&
                    emittedSequence > BigInteger.ZERO &&
                    emittedBlockHeight > BigInteger.ZERO &&
                    emittedEpoch > BigInteger.ZERO)
        ) {
            "$context ledger coordinates must all be zero or all be positive"
        }
        check(
            authorizationClaimBlockHeight == BigInteger.ZERO ||
                (emittedBlockHeight >= authorizationClaimBlockHeight &&
                    emittedEpoch >= authorizationClaimEpoch)
        ) {
            "$context emission coordinates must not precede authorization claim coordinates"
        }
        val outputArtifact = parseArtifact(
            expectObject(root["output_artifact"], "$context.output_artifact"),
            "$context.output_artifact",
            requiredRole = "output",
        )
        val outputReplicationOrderId = manifestDigest(
            root["output_replication_order_id"],
            "$context.output_replication_order_id",
        )
        check(
            outputReplicationOrderId == deriveSorafsAutoReplicationOrderIdV1(
                outputArtifact.sorafsManifestDigest,
            )
        ) {
            "$context.output_replication_order_id must equal the tagged automatic " +
                "replication-order ID derived from output_artifact.sorafs_manifest_digest"
        }
        return SoracloudPrivateUploadedModelExecutionReceipt(
            schemaVersion = schemaVersion(root["schema_version"], "$context.schema_version"),
            networkId = networkId(root["network_id"], "$context.network_id"),
            receiptId = hashLiteral(root["receipt_id"], "$context.receipt_id"),
            serviceName = canonicalServiceName(root["service_name"], "$context.service_name"),
            serviceVersion = requiredString(root["service_version"], "$context.service_version"),
            modelId = requiredString(root["model_id"], "$context.model_id"),
            weightVersion = requiredString(root["weight_version"], "$context.weight_version"),
            runtimeVersion = requiredString(root["runtime_version"], "$context.runtime_version"),
            modelManifestDigest = manifestDigest(
                root["model_manifest_digest"],
                "$context.model_manifest_digest",
            ),
            modelBundleRoot = hashLiteral(root["model_bundle_root"], "$context.model_bundle_root"),
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
            outputArtifact = outputArtifact,
            outputReplicationOrderId = outputReplicationOrderId,
            inputCommitment = hashLiteral(root["input_commitment"], "$context.input_commitment"),
            outputCommitment = hashLiteral(root["output_commitment"], "$context.output_commitment"),
            outputRecipient = parseOutputRecipient(
                expectObject(root["output_recipient"], "$context.output_recipient"),
                "$context.output_recipient",
            ),
            requestCommitment = hashLiteral(root["request_commitment"], "$context.request_commitment"),
            resultCommitment = hashLiteral(root["result_commitment"], "$context.result_commitment"),
            authorizationClaimBlockHeight = authorizationClaimBlockHeight,
            authorizationClaimEpoch = authorizationClaimEpoch,
            emittedSequence = emittedSequence,
            emittedBlockHeight = emittedBlockHeight,
            emittedEpoch = emittedEpoch,
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
            sorafsManifestDigest = manifestDigest(
                root["sorafs_manifest_digest"],
                "$context.sorafs_manifest_digest",
            ),
            sorafsRootCid = sorafsRootCid(root["sorafs_root_cid"], "$context.sorafs_root_cid"),
            artifactHash = hashLiteral(root["artifact_hash"], "$context.artifact_hash"),
            ciphertextBytes = boundedLong(
                root["ciphertext_bytes"],
                "$context.ciphertext_bytes",
                1L,
                SORACLOUD_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1,
            ),
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
        val validatorAccountId = requiredString(
            root["validator_account_id"],
            "$context.validator_account_id",
        )
        val peerId = requiredString(root["peer_id"], "$context.peer_id")
        try {
            requireSoracloudValidatorIdentity(validatorAccountId, peerId)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(
                "$context must bind a canonical universal single-signatory AccountId to its canonical PeerId",
                error,
            )
        }
        return SoracloudRuntimeDeterministicValidatorHost(
            laneId = boundedLong(root["lane_id"], "$context.lane_id", 0L, U32_MAX),
            validatorAccountId = validatorAccountId,
            peerId = peerId,
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
        val publicKeyBytesBase64 = canonicalBase64X25519Key(
            root["public_key_bytes"],
            "$context.public_key_bytes",
        )
        val publicKeyFingerprint = hashLiteral(
            root["public_key_fingerprint"],
            "$context.public_key_fingerprint",
        )
        val publicKeyBytes = try {
            decodeCanonicalSoracloudX25519PublicKey(publicKeyBytesBase64, "$context.public_key_bytes")
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$context.public_key_bytes is invalid", error)
        }
        val fingerprintBytes = try {
            requireSoracloudHashLiteral(publicKeyFingerprint, "$context.public_key_fingerprint")
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$context.public_key_fingerprint is invalid", error)
        }
        check(fingerprintBytes.contentEquals(IrohaHash.prehash(publicKeyBytes))) {
            "$context.public_key_fingerprint must equal IrohaHash.prehash(public_key_bytes)"
        }
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
            publicKeyBytesBase64 = publicKeyBytesBase64,
            publicKeyFingerprint = publicKeyFingerprint,
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
        return string
    }

    private fun optionalString(value: Any?, path: String): String? {
        if (value == null) return null
        check(value is String) { "$path must be a string" }
        check(value == value.trim()) { "$path must not contain leading or trailing whitespace" }
        check(value.none { Character.isISOControl(it) }) {
            "$path must be free of control characters"
        }
        return value
    }

    private fun optionalNonBlankString(value: Any?, path: String): String? =
        if (value == null) null else requiredString(value, path)

    private fun canonicalServiceName(value: Any?, path: String): String {
        val name = requiredString(value, path)
        try {
            requireCanonicalSoracloudName(name, path)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(error.message, error)
        }
        return name
    }

    private fun asLong(value: Any?, path: String): Long = JsonNumbers.asLong(value, path)

    private fun unsigned64Integer(value: Any?, path: String): BigInteger {
        val parsed = when (value) {
            is BigInteger -> value
            is Byte -> BigInteger.valueOf(value.toLong())
            is Short -> BigInteger.valueOf(value.toLong())
            is Int -> BigInteger.valueOf(value.toLong())
            is Long -> BigInteger.valueOf(value)
            else -> throw IllegalStateException("$path must be an integer")
        }
        check(parsed >= BigInteger.ZERO && parsed <= U64_MAX) {
            "$path must fit in unsigned 64-bit range"
        }
        return parsed
    }

    private fun boundedLong(value: Any?, path: String, minimum: Long, maximum: Long): Long {
        val parsed = asLong(value, path)
        check(parsed in minimum..maximum) { "$path must be within $minimum..=$maximum" }
        return parsed
    }

    private fun asOptionalBoundedLong(
        value: Any?,
        path: String,
        minimum: Long,
        maximum: Long,
    ): Long? = if (value == null) null else boundedLong(value, path, minimum, maximum)

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

    private fun manifestDigest(value: Any?, path: String): List<Int> {
        val values = asArray(value, path)
        check(values.size == 32) { "$path must contain exactly 32 unsigned integer bytes" }
        val bytes = values.mapIndexed { index, element ->
            boundedLong(element, "$path[$index]", 0L, 255L).toInt()
        }
        return Collections.unmodifiableList(ArrayList(bytes))
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

    private fun submissionPhase(
        value: Any?,
        path: String,
    ): SoracloudPrivateUploadedModelSubmissionPhase {
        val parsed = requiredString(value, path)
        return try {
            SoracloudPrivateUploadedModelSubmissionPhase.fromWireValue(parsed)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$path has an unknown first-release phase", error)
        }
    }

    private fun countMode(value: Any?, path: String): String {
        val parsed = requiredString(value, path)
        check(parsed == "bounded" || parsed == "exact") {
            "$path must equal `bounded` or `exact`"
        }
        return parsed
    }

    private fun requireTransactionHashMatchesPhase(
        submissionPhase: SoracloudPrivateUploadedModelSubmissionPhase,
        transactionHash: String?,
    ) {
        val required = SoracloudPrivateUploadedModelSubmissionPhase
            .requiresTransactionHash(submissionPhase)
        check(required == (transactionHash != null)) {
            if (required) {
                "soracloud private execute response.transaction_hash is required for " +
                    "`${submissionPhase.wireValue}`"
            } else {
                "soracloud private execute response.transaction_hash must be null for " +
                    "`${submissionPhase.wireValue}`"
            }
        }
    }

    private fun requireReceiptPersistenceMatchesPhase(
        submissionPhase: SoracloudPrivateUploadedModelSubmissionPhase,
        receipt: SoracloudPrivateUploadedModelExecutionReceipt,
    ) {
        val assigned =
            receipt.authorizationClaimBlockHeight > BigInteger.ZERO &&
                receipt.authorizationClaimEpoch > BigInteger.ZERO &&
                receipt.emittedSequence > BigInteger.ZERO &&
                receipt.emittedBlockHeight > BigInteger.ZERO &&
                receipt.emittedEpoch > BigInteger.ZERO
        val required = SoracloudPrivateUploadedModelSubmissionPhase
            .requiresAssignedReceipt(submissionPhase)
        check(required == assigned) {
            if (required) {
                "soracloud private execute response.receipt must use positive ledger coordinates " +
                    "for `committed`"
            } else {
                "soracloud private execute response.receipt must use zero ledger coordinates for " +
                "`${submissionPhase.wireValue}`"
            }
        }
    }

    private fun canonicalBase64X25519Key(value: Any?, path: String): String {
        val encoded = requiredString(value, path)
        try {
            decodeCanonicalSoracloudX25519PublicKey(encoded, path)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException("$path must be a canonical non-low-order X25519 key", error)
        }
        return encoded
    }

    private fun hashLiteral(value: Any?, path: String): String {
        val literal = requiredString(value, path)
        try {
            requireSoracloudHashLiteral(literal, path)
        } catch (error: IllegalArgumentException) {
            throw IllegalStateException(
                "$path must be an exact uppercase checksummed hash literal with marker bit",
                error,
            )
        }
        return literal
    }

    private fun optionalHashLiteral(value: Any?, path: String): String? =
        if (value == null) null else hashLiteral(value, path)

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
