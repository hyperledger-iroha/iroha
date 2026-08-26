package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Base64
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull
import org.bouncycastle.crypto.params.X25519PrivateKeyParameters
import org.hyperledger.iroha.sdk.address.AccountAddress
import org.hyperledger.iroha.sdk.address.MultisigMemberPayload
import org.hyperledger.iroha.sdk.address.MultisigPolicyPayload
import org.hyperledger.iroha.sdk.address.encodePublicKeyMultihash
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.crypto.IrohaHash
import org.hyperledger.iroha.sdk.testing.TestEd25519Keys

class SoracloudPrivateUploadedModelJsonParserTest {

    @Test
    fun parsesReceiptSubmittedPrivateExecuteResponseAndDurableReceipt() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )

        assertEquals(1L, response.schemaVersion)
        assertEquals(1L, response.status["schema_version"])
        assertEquals(
            "portal",
            (response.status["bundle"] as Map<*, *>)["service_name"],
        )
        assertEquals(
            "artifact-1",
            (response.status["artifact"] as Map<*, *>)["artifact_id"],
        )
        assertEquals(
            SoracloudPrivateUploadedModelSubmissionPhase.RECEIPT_SUBMITTED,
            response.submissionPhase,
        )
        assertEquals(TRANSACTION_HASH, response.transactionHash)
        assertEquals(NETWORK_ID, response.receipt.networkId)
        assertEquals(RECEIPT_ID, response.receipt.receiptId)
        assertEquals("portal", response.receipt.serviceName)
        assertEquals("2026.1", response.receipt.serviceVersion)
        assertEquals("decrypt-upload-1", response.receipt.decryptionRequestId)
        assertEquals(0L, response.receipt.attestingValidator.laneId)
        assertEquals(VALIDATOR_ACCOUNT_ID, response.receipt.attestingValidator.validatorAccountId)
        assertEquals(VALIDATOR_PEER_ID, response.receipt.attestingValidator.peerId)
        assertEquals("input", response.receipt.inputArtifact.artifactRole)
        assertEquals("output", response.receipt.outputArtifact.artifactRole)
        assertEquals(List(32) { 17 }, response.receipt.modelManifestDigest)
        assertEquals(OUTPUT_REPLICATION_ORDER_ID, response.receipt.outputReplicationOrderId)
        assertEquals(0x80, response.receipt.outputReplicationOrderId.first() and 0x80)
        assertEquals(List(32) { 34 }, response.receipt.inputArtifact.sorafsManifestDigest)
        assertEquals(36, response.receipt.inputArtifact.sorafsRootCid.size)
        assertEquals(listOf(1, 113, 31, 32), response.receipt.inputArtifact.sorafsRootCid.take(4))
        assertEquals(List(32) { 51 }, response.outputArtifact.sorafsManifestDigest)
        assertEquals("recipient-key", response.receipt.outputRecipient.keyId)
        assertEquals(32, response.receipt.outputRecipient.publicKeyBytes().size)
        assertEquals(BigInteger.ZERO, response.receipt.emittedSequence)
        assertEquals(BigInteger.ZERO, response.receipt.emittedBlockHeight)
    }

    @Test
    fun executeStatusRequiresExactV1EnvelopeAndOwnsNestedJson() {
        val parsed = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )
        @Suppress("UNCHECKED_CAST")
        val bundle = LinkedHashMap(parsed.status["bundle"] as Map<String, Any?>)
        val modalities = (bundle["modalities"] as List<Any?>).toMutableList()
        bundle["modalities"] = modalities
        @Suppress("UNCHECKED_CAST")
        val artifact = LinkedHashMap(parsed.status["artifact"] as Map<String, Any?>)
        val source = linkedMapOf<String, Any?>(
            "schema_version" to 1L,
            "bundle" to bundle,
            "artifact" to artifact,
        )
        val response = SoracloudPrivateUploadedModelExecuteResponse(
            schemaVersion = parsed.schemaVersion,
            status = source,
            submissionPhase = parsed.submissionPhase,
            transactionHash = parsed.transactionHash,
            receipt = parsed.receipt,
            outputArtifact = parsed.outputArtifact,
        )

        source["schema_version"] = 2L
        bundle["service_name"] = "mutated"
        modalities.add("mutated")
        artifact.clear()

        val statusBundle = response.status["bundle"] as Map<*, *>
        val statusModalities = statusBundle["modalities"] as List<*>
        val statusArtifact = response.status["artifact"] as Map<*, *>
        assertEquals(1L, response.status["schema_version"])
        assertEquals("portal", statusBundle["service_name"])
        assertEquals(listOf("text"), statusModalities)
        assertEquals("artifact-1", statusArtifact["artifact_id"])
        val erasedStatus: Any = response.status
        assertFailsWith<RuntimeException> {
            @Suppress("UNCHECKED_CAST")
            (erasedStatus as MutableMap<String, Any?>)["extra"] = true
        }
        assertFailsWith<RuntimeException> {
            @Suppress("UNCHECKED_CAST")
            (statusBundle as MutableMap<String, Any?>)["extra"] = true
        }
        assertFailsWith<RuntimeException> {
            @Suppress("UNCHECKED_CAST")
            (statusModalities as MutableList<Any?>).add("extra")
        }

        val nullableArtifact = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace(statusJson(), statusJson(artifact = "null"))
                .bytes()
        )
        assertNull(nullableArtifact.status["artifact"])

        for (invalid in listOf(
            "{}",
            """{"schema_version":2,"bundle":{},"artifact":null}""",
            """{"schema_version":1,"bundle":null,"artifact":null}""",
            """{"schema_version":1,"bundle":{},"artifact":[]}""",
            """{"schema_version":1,"bundle":{},"artifact":null,"legacy":true}""",
            statusJson().replace(
                "\"family\": \"decoder-only\"",
                "\"family\": \"decoder-only\", \"legacy\": true",
            ),
            statusJson().replace(
                "\"artifact_id\": \"artifact-1\"",
                "\"artifact_id\": \"artifact-1\", \"legacy\": true",
            ),
            statusJson().replace("\"modalities\": [\"text\"]", "\"modalities\": \"text\""),
        )) {
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson().replace(statusJson(), invalid).bytes()
                )
            }
        }

        for ((canonical, mismatched) in listOf(
            "\"service_name\": \"portal\"" to "\"service_name\": \"other\"",
            "\"model_id\": \"upload-1\"" to "\"model_id\": \"upload-2\"",
            "\"weight_version\": \"v1\"" to "\"weight_version\": \"v2\"",
            "\"bundle_root\": \"$MODEL_BUNDLE_ROOT\"" to
                "\"bundle_root\": \"$TRANSACTION_HASH\"",
            "\"sorafs_manifest_digest\": ${manifestDigestJson(17)}" to
                "\"sorafs_manifest_digest\": ${manifestDigestJson(18)}",
        )) {
            val mismatchedStatus = statusJson().replaceFirst(canonical, mismatched)
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson().replace(statusJson(), mismatchedStatus).bytes()
                )
            }
        }
    }

    @Test
    fun parsedManifestDigestsAreImmutableAndModelsEnforceCanonicalBytes() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )

        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                response.receipt,
                modelManifestDigest = SoracloudImmutableList.copyOf(List(31) { 17 })
            )
        }
        assertFailsWith<IllegalArgumentException> {
            response.receipt.inputArtifact.copy(
                sorafsManifestDigest = SoracloudImmutableList.copyOf(
                    List(32) { index -> if (index == 31) 256 else 34 }
                )
            )
        }
    }

    @Test
    fun directConstructorsDefensivelyOwnListsAndKeepStructuralEquality() {
        val parsed = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )
        val manifestSource = MutableList(32) { 34 }
        val rootSource = ROOT_CID_VALUES.toMutableList()
        val artifact = SoracloudPrivateModelArtifactRef(
            schemaVersion = 1,
            sorafsManifestDigest = manifestSource,
            sorafsRootCid = rootSource,
            artifactHash = INPUT_ARTIFACT_HASH,
            ciphertextBytes = 64,
            artifactRole = "input",
        )
        val artifactCopy = artifact.copy()

        manifestSource[0] = 99
        rootSource[4] = 99
        assertEquals(34, artifact.sorafsManifestDigest[0])
        assertEquals(1, artifact.sorafsRootCid[4])
        assertEquals(artifact, artifactCopy)
        assertEquals(artifact.hashCode(), artifactCopy.hashCode())
        assertFailsWith<IllegalArgumentException> {
            artifact.copy(artifactRole = "plaintext")
        }
        assertFailsWith<IllegalArgumentException> {
            artifact.copy(artifactHash = ZERO_PREHASH_SENTINEL)
        }

        val receiptManifestSource = MutableList(32) { 17 }
        val replicationOrderSource = OUTPUT_REPLICATION_ORDER_ID.toMutableList()
        val receipt = copyReceipt(
            parsed.receipt,
            modelManifestDigest = receiptManifestSource,
            outputReplicationOrderId = replicationOrderSource,
            emittedSequence = BigInteger.ONE,
            emittedBlockHeight = BigInteger.ONE,
        )
        receiptManifestSource[0] = 99
        replicationOrderSource[0] = 99
        assertEquals(17, receipt.modelManifestDigest[0])
        assertEquals(OUTPUT_REPLICATION_ORDER_ID[0], receipt.outputReplicationOrderId[0])
        val receiptCopy = copyReceipt(receipt)
        assertEquals(receipt, receiptCopy)
        assertEquals(receipt.hashCode(), receiptCopy.hashCode())
        assertEquals(receipt.toString(), receiptCopy.toString())
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                receipt,
                outputReplicationOrderId = MISMATCHING_OUTPUT_REPLICATION_ORDER_ID,
            )
        }

        val responseCopy = SoracloudPrivateUploadedModelExecuteResponse(
            schemaVersion = parsed.schemaVersion,
            status = parsed.status,
            submissionPhase = parsed.submissionPhase,
            transactionHash = parsed.transactionHash,
            receipt = parsed.receipt,
            outputArtifact = parsed.outputArtifact,
        )
        assertEquals(parsed, responseCopy)
        assertEquals(parsed.hashCode(), responseCopy.hashCode())
        assertEquals(parsed.toString(), responseCopy.toString())
        check("receipt_submitted" in responseCopy.toString())

        val receiptSource = mutableListOf(receipt)
        val receiptList = SoracloudPrivateUploadedModelReceiptListResponse(
            schemaVersion = 1,
            receipts = receiptSource,
            total = 1,
            returnedItems = 1,
            remainingItems = 0,
            hasMore = false,
            countMode = "exact",
            continueCursor = null,
        )
        receiptSource.clear()
        assertEquals(listOf(receipt), receiptList.receipts)
        assertEquals(receiptList, receiptList.copy())
    }

    @Test
    fun parsesCommittedReplayWithExplicitNullTransactionHash() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace("\"submission_phase\": \"receipt_submitted\"", "\"submission_phase\": \"committed\"")
                .replace("\"transaction_hash\": \"$TRANSACTION_HASH\"", "\"transaction_hash\": null")
                .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": 17")
                .replace("\"emitted_block_height\": 0", "\"emitted_block_height\": 501")
                .bytes()
        )

        assertEquals(
            SoracloudPrivateUploadedModelSubmissionPhase.COMMITTED,
            response.submissionPhase,
        )
        assertNull(response.transactionHash)
        assertEquals(BigInteger.valueOf(17L), response.receipt.emittedSequence)
        assertEquals(BigInteger.valueOf(501L), response.receipt.emittedBlockHeight)
    }

    @Test
    fun parsesEveryUncommittedFirstReleaseSubmissionPhase() {
        val awaiting = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace(
                    "\"submission_phase\": \"receipt_submitted\"",
                    "\"submission_phase\": \"awaiting_output_durability\"",
                )
                .replace("\"transaction_hash\": \"$TRANSACTION_HASH\"", "\"transaction_hash\": null")
                .bytes()
        )
        assertEquals(
            SoracloudPrivateUploadedModelSubmissionPhase.AWAITING_OUTPUT_DURABILITY,
            awaiting.submissionPhase,
        )
        assertNull(awaiting.transactionHash)

        val pinSubmitted = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace(
                    "\"submission_phase\": \"receipt_submitted\"",
                    "\"submission_phase\": \"output_pin_submitted\"",
                )
                .bytes()
        )
        assertEquals(
            SoracloudPrivateUploadedModelSubmissionPhase.OUTPUT_PIN_SUBMITTED,
            pinSubmitted.submissionPhase,
        )
        assertEquals(TRANSACTION_HASH, pinSubmitted.transactionHash)
    }

    @Test
    fun parsesFullUnsigned64ReceiptCoordinatesAndRejectsNonIntegersOrOverflow() {
        val committedAtU64Max = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace("\"submission_phase\": \"receipt_submitted\"", "\"submission_phase\": \"committed\"")
                .replace("\"transaction_hash\": \"$TRANSACTION_HASH\"", "\"transaction_hash\": null")
                .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": $U64_MAX")
                .replace("\"emitted_block_height\": 0", "\"emitted_block_height\": $U64_MAX")
                .bytes()
        )

        assertEquals(U64_MAX, committedAtU64Max.receipt.emittedSequence)
        assertEquals(U64_MAX, committedAtU64Max.receipt.emittedBlockHeight)
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                committedAtU64Max.receipt,
                emittedSequence = U64_MAX.add(BigInteger.ONE),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                committedAtU64Max.receipt,
                emittedBlockHeight = BigInteger.ZERO,
            )
        }

        for (invalid in listOf("-1", "1.0", "1e0", U64_MAX.add(BigInteger.ONE).toString())) {
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson()
                        .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": $invalid")
                        .bytes()
                )
            }
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson()
                        .replace(
                            "\"emitted_block_height\": 0",
                            "\"emitted_block_height\": $invalid",
                        )
                        .bytes()
                )
            }
        }
    }

    @Test
    fun parsesPrivateReceiptListPaginationMetadata() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            """
                {
                  "schema_version": 1,
                  "receipts": [${receiptJson()
                      .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": 17")
                      .replace("\"emitted_block_height\": 0", "\"emitted_block_height\": 501")}],
                  "total": 3,
                  "returned_items": 1,
                  "remaining_items": 2,
                  "has_more": true,
                  "count_mode": "exact",
                  "continue_cursor": "$RECEIPT_CURSOR"
                }
            """.trimIndent().bytes()
        )

        assertEquals(1, response.receipts.size)
        assertEquals(3L, response.total)
        assertEquals(1L, response.returnedItems)
        assertEquals(2L, response.remainingItems)
        assertEquals("exact", response.countMode)
        assertEquals(RECEIPT_CURSOR, response.continueCursor)
        assertEquals("2026.1", response.receipts.single().serviceVersion)
    }

    @Test
    fun boundedReceiptListAcceptsRequiredNullableFields() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            """
                {
                  "schema_version": 1,
                  "receipts": [],
                  "total": null,
                  "returned_items": 0,
                  "remaining_items": null,
                  "has_more": false,
                  "count_mode": "bounded",
                  "continue_cursor": null
                }
            """.trimIndent().bytes()
        )

        assertNull(response.total)
        assertNull(response.remainingItems)
        assertFalse(response.hasMore)
        assertNull(response.continueCursor)
    }

    @Test
    fun rejectsRetiredInstructionSurfaceAndInvalidSubmissionState() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"receipt\":", "\"tx_instructions\": [],\n  \"receipt\":")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"submission_phase\"", "\"submission_status\"")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\": \"receipt_submitted\"",
                        "\"submission_phase\": \"pending\"",
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"transaction_hash\": \"$TRANSACTION_HASH\"", "\"transaction_hash\": null")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\": \"receipt_submitted\"",
                        "\"submission_phase\": \"awaiting_output_durability\"",
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\": \"receipt_submitted\"",
                        "\"submission_phase\": \"output_pin_submitted\"",
                    )
                    .replace("\"transaction_hash\": \"$TRANSACTION_HASH\"", "\"transaction_hash\": null")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"submission_phase\": \"receipt_submitted\"",
                        "\"submission_phase\": \"committed\"",
                    )
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsMissingProductionReceiptEvidence() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"network_id\": \"$NETWORK_ID\",", "").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"service_version\": \"2026.1\",", "").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"decryption_request_id\": \"decrypt-upload-1\",", "")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"output_replication_order_id\": ${bytesJson(OUTPUT_REPLICATION_ORDER_ID)},",
                        "",
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(Regex(",\\s*\"emitted_block_height\": 0"), "")
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsNonCanonicalReceiptNetworkIdentity() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"network_id\": \"$NETWORK_ID\"", "\"network_id\": 7").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(NETWORK_ID, NETWORK_ID.lowercase()).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(NETWORK_ID, NETWORK_ID.replace("#A2F0", "#A2F1")).bytes()
            )
        }
    }

    @Test
    fun rejectsMalformedValidatorAndOutputRecipient() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"lane_id\": 0", "\"lane_id\": -1").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(VALIDATOR_ACCOUNT_ID, "validator@public").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(VALIDATOR_PEER_ID, "ed25519:$VALIDATOR_PEER_ID").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(VALIDATOR_PEER_ID, OTHER_VALIDATOR_PEER_ID).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(VALIDATOR_ACCOUNT_ID, MULTISIG_VALIDATOR_ACCOUNT_ID).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(PUBLIC_KEY_BASE64, "not-base64").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(PUBLIC_KEY_BASE64, ZERO_X25519_KEY_BASE64).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("X25519HkdfSha256", "UnknownKem").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"value\": null", "\"value\": {}").bytes()
            )
        }
    }

    @Test
    fun rejectsNonCanonicalHashFieldsAndRecipientFingerprintMismatch() {
        val canonicalHashes = listOf(
            "transaction_hash" to TRANSACTION_HASH,
            "receipt_id" to RECEIPT_ID,
            "model_bundle_root" to MODEL_BUNDLE_ROOT,
            "artifact_hash" to INPUT_ARTIFACT_HASH,
            "artifact_hash" to OUTPUT_ARTIFACT_HASH,
            "input_commitment" to INPUT_COMMITMENT,
            "output_commitment" to OUTPUT_COMMITMENT,
            "request_commitment" to REQUEST_COMMITMENT,
            "result_commitment" to RESULT_COMMITMENT,
            "public_key_fingerprint" to PUBLIC_KEY_FINGERPRINT,
        )
        for ((field, canonical) in canonicalHashes) {
            assertHashFieldRejected(field, canonical, "not-a-hash")
        }
        assertHashFieldRejected("receipt_id", RECEIPT_ID, RECEIPT_ID.lowercase())
        assertHashFieldRejected("receipt_id", RECEIPT_ID, tamperChecksum(RECEIPT_ID))
        assertHashFieldRejected("receipt_id", RECEIPT_ID, UNMARKED_HASH_LITERAL)
        assertHashFieldRejected("receipt_id", RECEIPT_ID, ZERO_PREHASH_SENTINEL)
        assertHashFieldRejected(
            "public_key_fingerprint",
            PUBLIC_KEY_FINGERPRINT,
            RESULT_COMMITMENT,
        )

        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )
        assertFailsWith<IllegalArgumentException> {
            response.receipt.outputRecipient.copy(publicKeyFingerprint = RESULT_COMMITMENT)
        }
    }

    @Test
    fun rejectsLeadingOrTrailingWhitespaceWithoutNormalization() {
        for ((canonical, padded) in listOf(
            "\"submission_phase\": \"receipt_submitted\"" to
                "\"submission_phase\": \"receipt_submitted \"",
            "\"service_version\": \"2026.1\"" to
                "\"service_version\": \" 2026.1\"",
            "\"validator_account_id\": \"$VALIDATOR_ACCOUNT_ID\"" to
                "\"validator_account_id\": \" $VALIDATOR_ACCOUNT_ID\"",
            "\"peer_id\": \"$VALIDATOR_PEER_ID\"" to
                "\"peer_id\": \"$VALIDATOR_PEER_ID \"",
        )) {
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson().replace(canonical, padded).bytes()
                )
            }
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson()
                    .replace("\"count_mode\": \"exact\"", "\"count_mode\": \" exact\"")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"service_version\": \"2026.1\"",
                        "\"service_version\": \"2026\\n1\"",
                    )
                    .bytes()
            )
        }
    }

    @Test
    fun enforcesExactServiceName() {
        val composed = "caf\u00e9"
        val decomposed = "cafe\u0301"
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace("\"service_name\": \"portal\"", "\"service_name\": \"$composed\"")
                .bytes()
        )
        assertEquals(composed, response.receipt.serviceName)
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"service_name\": \"portal\"",
                        "\"service_name\": \"$decomposed\"",
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        "\"service_name\": \"portal\"",
                        "\"service_name\": \"portal#alias\"",
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(response.receipt, serviceName = decomposed)
        }
        for (nonBreakingSpace in listOf('\u00a0', '\u2007', '\u202f')) {
            assertFailsWith<IllegalStateException> {
                SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                    executeResponseJson()
                        .replace(
                            "\"service_name\": \"portal\"",
                            "\"service_name\": \"por${nonBreakingSpace}tal\"",
                        )
                        .bytes()
                )
            }
        }
    }

    @Test
    fun enforcesUploadedModelIdRules() {
        val receipt = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        ).receipt
        val maximumIdentifier = "a".repeat(128)

        assertEquals(maximumIdentifier, copyReceipt(receipt, modelId = maximumIdentifier).modelId)
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(receipt, modelId = "a".repeat(129))
        }
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(receipt, modelId = "mod\u00e9l")
        }
    }

    @Test
    fun enforcesUploadedModelWeightVersionRules() {
        val receipt = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        ).receipt
        val maximumIdentifier = "v".repeat(128)

        assertEquals(
            maximumIdentifier,
            copyReceipt(receipt, weightVersion = maximumIdentifier).weightVersion,
        )
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(receipt, weightVersion = "v".repeat(129))
        }
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(receipt, weightVersion = "weight/version")
        }
    }

    @Test
    fun enforcesUploadedModelServiceVersionUtf8Bound() {
        val receipt = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        ).receipt
        val maximumServiceVersion = "\u00e9".repeat(128)

        assertEquals(
            maximumServiceVersion,
            copyReceipt(receipt, serviceVersion = maximumServiceVersion).serviceVersion,
        )
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(receipt, serviceVersion = maximumServiceVersion + "a")
        }
    }

    @Test
    fun rejectsMismatchedResponseOutputArtifact() {
        val canonical = executeResponseJson()
        val target = manifestDigestJson(51)
        val responseOutput = canonical.lastIndexOf(target)
        val mismatched = canonical.substring(0, responseOutput) +
            manifestDigestJson(52) +
            canonical.substring(responseOutput + target.length)

        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(mismatched.bytes())
        }
    }

    @Test
    fun rejectsNegativeReceiptPaginationMetadata() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "-1").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(returnedItems = "-1").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(remainingItems = "-1").bytes()
            )
        }
    }

    @Test
    fun rejectsReceiptPaginationMetadataAboveU32() {
        val aboveU32 = "4294967296"
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = aboveU32).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(returnedItems = aboveU32).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(remainingItems = aboveU32).bytes()
            )
        }
    }

    @Test
    fun rejectsNonCanonicalOrContradictoryReceiptCountMetadata() {
        val exact = receiptListJson()
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                exact.replace("\"count_mode\": \"exact\"", "\"count_mode\": \"EXACT\"").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                exact.replace("\"count_mode\": \"exact\"", "\"count_mode\": \"full\"").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "null").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                exact.replace("\"count_mode\": \"exact\"", "\"count_mode\": \"bounded\"").bytes()
            )
        }
    }

    @Test
    fun rejectsContradictoryReceiptPaginationRelationshipsAndAcceptsSaturation() {
        val receipt = committedReceiptJson(
            RECEIPT_ID,
            emittedSequence = 1,
            emittedBlockHeight = 101,
        )
        assertFailsWith<IllegalArgumentException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "1", returnedItems = "1", remainingItems = "1")
                    .replace("\"receipts\": []", "\"receipts\": [$receipt]")
                    .let(::withMore)
                    .bytes()
            )
        }

        val saturated = SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            receiptListJson(
                total = U32_MAX.toString(),
                remainingItems = U32_MAX.toString(),
            ).let(::withMore)
                .bytes()
        )
        assertEquals(U32_MAX, saturated.total)
    }

    @Test
    fun rejectsReceiptListEntriesWithoutPositiveLedgerCoordinates() {
        val zeroCoordinatesReceipt = receiptJson()
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "1", returnedItems = "1")
                    .replace("\"receipts\": []", "\"receipts\": [$zeroCoordinatesReceipt]")
                    .bytes()
            )
        }

        val positiveSequenceOnlyReceipt = receiptJson()
            .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": 1")
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "1", returnedItems = "1")
                    .replace(
                        "\"receipts\": []",
                        "\"receipts\": [$positiveSequenceOnlyReceipt]",
                    )
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsNonCanonicalReceiptListOrderAndDuplicates() {
        val first = committedReceiptJson(RECEIPT_ID, emittedSequence = 1, emittedBlockHeight = 101)
        val second = committedReceiptJson(
            MODEL_BUNDLE_ROOT,
            emittedSequence = 2,
            emittedBlockHeight = 102,
        )
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "2", returnedItems = "2")
                    .replace("\"receipts\": []", "\"receipts\": [$second, $first]")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "2", returnedItems = "2")
                    .replace("\"receipts\": []", "\"receipts\": [$first, $first]")
                    .bytes()
            )
        }

        val higherId = committedReceiptJson(
            MODEL_BUNDLE_ROOT,
            emittedSequence = 1,
            emittedBlockHeight = 101,
        )
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "2", returnedItems = "2")
                    .replace("\"receipts\": []", "\"receipts\": [$higherId, $first]")
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsReceiptListsMissingRequiredNullableKeys() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "null")
                    .replace(Regex("\\s*\"total\": null,"), "")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson()
                    .replace(Regex(",\\s*\"continue_cursor\": null"), "")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson()
                    .replace(Regex("\\s*\"remaining_items\": 0,"), "")
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsNonCanonicalManifestDigestsAndMismatchedReplicationOrderIds() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(manifestDigestJson(17), manifestDigestJson(17, size = 31))
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        bytesJson(OUTPUT_REPLICATION_ORDER_ID),
                        bytesJson(OUTPUT_REPLICATION_ORDER_ID.dropLast(1)),
                    )
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(
                        bytesJson(OUTPUT_REPLICATION_ORDER_ID),
                        bytesJson(MISMATCHING_OUTPUT_REPLICATION_ORDER_ID),
                    )
                    .bytes()
            )
        }
        val parsed = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                parsed.receipt,
                outputReplicationOrderId = OUTPUT_REPLICATION_ORDER_ID.dropLast(1),
            )
        }
        assertFailsWith<IllegalArgumentException> {
            copyReceipt(
                parsed.receipt,
                outputReplicationOrderId = MISMATCHING_OUTPUT_REPLICATION_ORDER_ID,
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(manifestDigestJson(34), "\"input-manifest\"")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(manifestDigestJson(34), manifestDigestJson(34).replaceFirst("34", "256"))
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace(manifestDigestJson(51), manifestDigestJson(51).replaceFirst("51", "51.0"))
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsInvalidReceiptArtifactAndSequenceFields() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace("\"sorafs_root_cid\": $ROOT_CID_JSON,", "").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(ROOT_CID_JSON, "[1, 113, 31, 32, 1]").bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(ROOT_CID_JSON, ROOT_CID_JSON.replaceFirst("[1, 113", "[2, 113")).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(ROOT_CID_JSON, ZERO_DIGEST_ROOT_CID_JSON).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson().replace(ROOT_CID_JSON, ROOT_CID_JSON.replaceFirst(", 1, 2", ", 1.0, 2")).bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"ciphertext_bytes\": 64", "\"ciphertext_bytes\": 0")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": -1")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"emitted_block_height\": 0", "\"emitted_block_height\": -1")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"artifact_role\": \"input\"", "\"artifact_role\": \"output\"")
                    .bytes()
            )
        }
    }

    @Test
    fun rejectsArtifactCiphertextBytesAboveFirstReleaseMaximum() {
        val oversized = SORACLOUD_PRIVATE_MODEL_ENCRYPTED_ARTIFACT_MAX_BYTES_V1 + 1L
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"ciphertext_bytes\": 64", "\"ciphertext_bytes\": $oversized")
                    .bytes()
            )
        }

        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )
        assertFailsWith<IllegalArgumentException> {
            response.receipt.inputArtifact.copy(ciphertextBytes = oversized)
        }
    }

    @Test
    fun rejectsBlankReceiptIdentityFields() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"receipt_id\": \"$RECEIPT_ID\"", "\"receipt_id\": \"   \"")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"policy_id\": \"policy-1\"", "\"policy_id\": \"\"")
                    .bytes()
            )
        }
    }

    private fun assertHashFieldRejected(
        field: String,
        canonical: String,
        replacement: String,
    ) {
        val fixture = executeResponseJson()
        val canonicalField = "\"$field\": \"$canonical\""
        check(canonicalField in fixture) { "missing canonical fixture field $field" }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                fixture.replace(canonicalField, "\"$field\": \"$replacement\"").bytes()
            )
        }
    }

    private fun tamperChecksum(literal: String): String =
        literal.dropLast(1) + if (literal.last() == '0') '1' else '0'

    private fun copyReceipt(
        source: SoracloudPrivateUploadedModelExecutionReceipt,
        modelManifestDigest: List<Int> = source.modelManifestDigest,
        outputReplicationOrderId: List<Int> = source.outputReplicationOrderId,
        serviceName: String = source.serviceName,
        serviceVersion: String = source.serviceVersion,
        modelId: String = source.modelId,
        weightVersion: String = source.weightVersion,
        emittedSequence: BigInteger = source.emittedSequence,
        emittedBlockHeight: BigInteger = source.emittedBlockHeight,
    ): SoracloudPrivateUploadedModelExecutionReceipt =
        SoracloudPrivateUploadedModelExecutionReceipt(
            schemaVersion = source.schemaVersion,
            networkId = source.networkId,
            receiptId = source.receiptId,
            serviceName = serviceName,
            serviceVersion = serviceVersion,
            modelId = modelId,
            weightVersion = weightVersion,
            runtimeVersion = source.runtimeVersion,
            modelManifestDigest = modelManifestDigest,
            modelBundleRoot = source.modelBundleRoot,
            policyId = source.policyId,
            decryptionRequestId = source.decryptionRequestId,
            attestingValidator = source.attestingValidator,
            inputArtifact = source.inputArtifact,
            outputArtifact = source.outputArtifact,
            outputReplicationOrderId = outputReplicationOrderId,
            inputCommitment = source.inputCommitment,
            outputCommitment = source.outputCommitment,
            outputRecipient = source.outputRecipient,
            requestCommitment = source.requestCommitment,
            resultCommitment = source.resultCommitment,
            emittedSequence = emittedSequence,
            emittedBlockHeight = emittedBlockHeight,
        )

    private fun receiptListJson(
        total: String = "0",
        returnedItems: String = "0",
        remainingItems: String = "0",
    ): String =
        """
            {
              "schema_version": 1,
              "receipts": [],
              "total": $total,
              "returned_items": $returnedItems,
              "remaining_items": $remainingItems,
              "has_more": false,
              "count_mode": "exact",
              "continue_cursor": null
            }
        """.trimIndent()

    private fun withMore(json: String): String = json
        .replace("\"has_more\": false", "\"has_more\": true")
        .replace("\"continue_cursor\": null", "\"continue_cursor\": \"$RECEIPT_CURSOR\"")

    private fun committedReceiptJson(
        receiptId: String,
        emittedSequence: Long,
        emittedBlockHeight: Long,
    ): String = receiptJson()
        .replace("\"receipt_id\": \"$RECEIPT_ID\"", "\"receipt_id\": \"$receiptId\"")
        .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": $emittedSequence")
        .replace(
            "\"emitted_block_height\": 0",
            "\"emitted_block_height\": $emittedBlockHeight",
        )

    private fun executeResponseJson(): String =
        """
            {
              "schema_version": 1,
              "status": ${statusJson()},
              "submission_phase": "receipt_submitted",
              "transaction_hash": "$TRANSACTION_HASH",
              "receipt": ${receiptJson()},
              "output_artifact": ${outputArtifactJson()}
            }
        """.trimIndent()

    private fun statusJson(
        artifact: String = artifactStatusJson(),
    ): String =
        """
            {
              "schema_version": 1,
              "bundle": {
                "schema_version": 1,
                "service_name": "portal",
                "model_id": "upload-1",
                "weight_version": "v1",
                "family": "decoder-only",
                "modalities": ["text"],
                "plaintext_root": "$RESULT_COMMITMENT",
                "runtime_format": {"runtime_format":"DeterministicQuantizedCpuV1","value":null},
                "bundle_root": "$MODEL_BUNDLE_ROOT",
                "sorafs_manifest_digest": ${manifestDigestJson(17)},
                "chunk_count": 1,
                "plaintext_bytes": 32,
                "ciphertext_bytes": 48,
                "chunk_manifest_root": "$CHUNK_MANIFEST_ROOT",
                "upload_recipient": {
                  "schema_version": 1,
                  "key_id": "bundle-key",
                  "key_version": 1,
                  "kem": {"kem":"X25519HkdfSha256","value":null},
                  "aead": {"aead":"Aes256Gcm","value":null},
                  "public_key_bytes": "$PUBLIC_KEY_BASE64",
                  "public_key_fingerprint": "$PUBLIC_KEY_FINGERPRINT"
                },
                "wrapped_bundle_key": {
                  "schema_version": 1,
                  "recipient_key_id": "bundle-key",
                  "recipient_key_version": 1,
                  "kem": {"kem":"X25519HkdfSha256","value":null},
                  "aead": {"aead":"Aes256Gcm","value":null},
                  "ephemeral_public_key": "$PUBLIC_KEY_BASE64",
                  "nonce": "$WRAPPED_NONCE_BASE64",
                  "wrapped_key_ciphertext": "$WRAPPED_KEY_CIPHERTEXT_BASE64",
                  "ciphertext_hash": "$WRAPPED_KEY_CIPHERTEXT_HASH",
                  "aad_digest": "$WRAPPED_KEY_AAD_DIGEST"
                },
                "pricing_policy": {"storage_price":"1"},
                "decryption_policy_ref": "policy-1"
              },
              "artifact": $artifact
            }
        """.trimIndent()

    private fun artifactStatusJson(): String =
        """
            {
              "service_name": "portal",
              "model_name": "portal_model",
              "artifact_id": "artifact-1",
              "training_job_id": "upload-1",
              "weight_version": "v1",
              "weight_artifact_hash": "$INPUT_ARTIFACT_HASH",
              "dataset_ref": "dataset:upload-1",
              "training_config_hash": "$INPUT_COMMITMENT",
              "reproducibility_hash": "$OUTPUT_COMMITMENT",
              "provenance_attestation_hash": "$REQUEST_COMMITMENT",
              "registered_sequence": 1,
              "consumed_by_version": "v1",
              "chunk_manifest_root": "$CHUNK_MANIFEST_ROOT"
            }
        """.trimIndent()

    private fun receiptJson(): String =
        """
            {
              "schema_version": 1,
              "network_id": "$NETWORK_ID",
              "receipt_id": "$RECEIPT_ID",
              "service_name": "portal",
              "service_version": "2026.1",
              "model_id": "upload-1",
              "weight_version": "v1",
              "runtime_version": "soracloud.quantized-cpu.v1",
              "model_manifest_digest": ${manifestDigestJson(17)},
              "model_bundle_root": "$MODEL_BUNDLE_ROOT",
              "policy_id": "policy-1",
              "decryption_request_id": "decrypt-upload-1",
              "attesting_validator": {
                "lane_id": 0,
                "validator_account_id": "$VALIDATOR_ACCOUNT_ID",
                "peer_id": "$VALIDATOR_PEER_ID"
              },
              "input_artifact": {
                "schema_version": 1,
                "sorafs_manifest_digest": ${manifestDigestJson(34)},
                "sorafs_root_cid": $ROOT_CID_JSON,
                "artifact_hash": "$INPUT_ARTIFACT_HASH",
                "ciphertext_bytes": 64,
                "artifact_role": "input"
              },
              "output_artifact": ${outputArtifactJson()},
              "output_replication_order_id": ${bytesJson(OUTPUT_REPLICATION_ORDER_ID)},
              "input_commitment": "$INPUT_COMMITMENT",
              "output_commitment": "$OUTPUT_COMMITMENT",
              "output_recipient": {
                "schema_version": 1,
                "key_id": "recipient-key",
                "key_version": 1,
                "kem": {"kem": "X25519HkdfSha256", "value": null},
                "aead": {"aead": "Aes256Gcm", "value": null},
                "public_key_bytes": "$PUBLIC_KEY_BASE64",
                "public_key_fingerprint": "$PUBLIC_KEY_FINGERPRINT"
              },
              "request_commitment": "$REQUEST_COMMITMENT",
              "result_commitment": "$RESULT_COMMITMENT",
              "emitted_sequence": 0,
              "emitted_block_height": 0
            }
        """.trimIndent()

    private fun outputArtifactJson(): String =
        """
            {
              "schema_version": 1,
              "sorafs_manifest_digest": ${manifestDigestJson(51)},
              "sorafs_root_cid": $ROOT_CID_JSON,
              "artifact_hash": "$OUTPUT_ARTIFACT_HASH",
              "ciphertext_bytes": 96,
              "artifact_role": "output"
            }
        """.trimIndent()

    private fun manifestDigestJson(value: Int, size: Int = 32): String =
        List(size) { value }.joinToString(prefix = "[", postfix = "]")

    private fun bytesJson(values: List<Int>): String =
        values.joinToString(prefix = "[", postfix = "]")

    private fun String.bytes(): ByteArray = toByteArray(StandardCharsets.UTF_8)

    private companion object {
        const val U32_MAX = 4_294_967_295L
        val U64_MAX: BigInteger = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        const val NETWORK_ID = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        const val UNMARKED_HASH_LITERAL = "hash:0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A0A#86CD"
        const val ZERO_PREHASH_SENTINEL = "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
        const val ROOT_CID_JSON = "[1, 113, 31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]"
        const val ZERO_DIGEST_ROOT_CID_JSON = "[1, 113, 31, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]"
        val ROOT_CID_VALUES = listOf(1, 113, 31, 32) + (1..32)
        val TRANSACTION_HASH = hashLiteral(0x01)
        val RECEIPT_ID = hashLiteral(0x02)
        val MODEL_BUNDLE_ROOT = hashLiteral(0x03)
        val INPUT_ARTIFACT_HASH = hashLiteral(0x04)
        val OUTPUT_ARTIFACT_HASH = hashLiteral(0x05)
        val INPUT_COMMITMENT = hashLiteral(0x06)
        val OUTPUT_COMMITMENT = hashLiteral(0x07)
        val REQUEST_COMMITMENT = hashLiteral(0x08)
        val RESULT_COMMITMENT = hashLiteral(0x09)
        val CHUNK_MANIFEST_ROOT = hashLiteral(0x0a)
        val WRAPPED_KEY_AAD_DIGEST = hashLiteral(0x0d)
        val OUTPUT_REPLICATION_ORDER_ID = listOf(
            223, 84, 153, 93, 189, 208, 15, 57,
            18, 144, 6, 143, 35, 114, 49, 183,
            235, 169, 151, 26, 48, 191, 231, 173,
            2, 235, 241, 47, 189, 13, 37, 69,
        )
        val MISMATCHING_OUTPUT_REPLICATION_ORDER_ID =
            OUTPUT_REPLICATION_ORDER_ID.toMutableList().also { values ->
                values[31] = values[31] xor 1
            }
        val VALIDATOR_PUBLIC_KEY = TestEd25519Keys.publicKey(0x30)
        val VALIDATOR_ACCOUNT_ID = AccountAddress
            .fromAccount(VALIDATOR_PUBLIC_KEY, "ed25519")
            .toI105Default()
        val VALIDATOR_PEER_ID = encodePublicKeyMultihash(0x01, VALIDATOR_PUBLIC_KEY)
        val OTHER_VALIDATOR_PEER_ID = encodePublicKeyMultihash(
            0x01,
            TestEd25519Keys.publicKey(0x31),
        )
        val MULTISIG_VALIDATOR_ACCOUNT_ID = AccountAddress.fromMultisigPolicy(
            MultisigPolicyPayload.of(
                version = 1,
                threshold = 1,
                members = listOf(MultisigMemberPayload(0x01, 1, VALIDATOR_PUBLIC_KEY)),
            )
        ).toI105Default()
        val PUBLIC_KEY_BYTES = X25519PrivateKeyParameters(
            ByteArray(32) { index -> (index + 1).toByte() },
            0,
        ).generatePublicKey().encoded
        val PUBLIC_KEY_BASE64 = Base64.getEncoder().encodeToString(PUBLIC_KEY_BYTES)
        val PUBLIC_KEY_FINGERPRINT = HashLiteral.canonicalize(IrohaHash.prehash(PUBLIC_KEY_BYTES))
        val WRAPPED_NONCE_BASE64 = Base64.getEncoder().encodeToString(ByteArray(12) { 0x0b })
        val WRAPPED_KEY_CIPHERTEXT = ByteArray(48) { 0x0c }
        val WRAPPED_KEY_CIPHERTEXT_BASE64 =
            Base64.getEncoder().encodeToString(WRAPPED_KEY_CIPHERTEXT)
        val WRAPPED_KEY_CIPHERTEXT_HASH =
            HashLiteral.canonicalize(IrohaHash.prehash(WRAPPED_KEY_CIPHERTEXT))
        val RECEIPT_CURSOR = "A".repeat(114)
        val ZERO_X25519_KEY_BASE64 = Base64.getEncoder().encodeToString(ByteArray(32))

        private fun hashLiteral(seed: Int): String =
            HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })
    }
}
