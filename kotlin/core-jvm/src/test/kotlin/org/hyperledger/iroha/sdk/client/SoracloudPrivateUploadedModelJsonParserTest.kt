package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNull

class SoracloudPrivateUploadedModelJsonParserTest {

    @Test
    fun parsesSubmittedPrivateExecuteResponseAndDurableReceipt() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().bytes()
        )

        assertEquals(1L, response.schemaVersion)
        assertEquals("finalized", response.status["status"])
        assertEquals("submitted", response.submissionStatus)
        assertEquals("transaction-hash", response.transactionHash)
        assertEquals(NETWORK_ID, response.receipt.networkId)
        assertEquals("receipt-1", response.receipt.receiptId)
        assertEquals("portal", response.receipt.serviceName)
        assertEquals("2026.1", response.receipt.serviceVersion)
        assertEquals("decrypt-upload-1", response.receipt.decryptionRequestId)
        assertEquals(0L, response.receipt.attestingValidator.laneId)
        assertEquals("validator@public", response.receipt.attestingValidator.validatorAccountId)
        assertEquals("peer-1", response.receipt.attestingValidator.peerId)
        assertEquals("input", response.receipt.inputArtifact.artifactRole)
        assertEquals("output", response.receipt.outputArtifact.artifactRole)
        assertEquals(36, response.receipt.inputArtifact.sorafsRootCid.size)
        assertEquals(listOf(1, 113, 31, 32), response.receipt.inputArtifact.sorafsRootCid.take(4))
        assertEquals("output-manifest", response.outputArtifact.sorafsManifestDigest)
        assertEquals("recipient-key", response.receipt.outputRecipient.keyId)
        assertEquals(32, response.receipt.outputRecipient.publicKeyBytes().size)
        assertEquals(0L, response.receipt.emittedSequence)
        assertEquals(0L, response.receipt.emittedBlockHeight)
    }

    @Test
    fun parsesCommittedReplayWithExplicitNullTransactionHash() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson()
                .replace("\"submission_status\": \"submitted\"", "\"submission_status\": \"committed\"")
                .replace("\"transaction_hash\": \"transaction-hash\"", "\"transaction_hash\": null")
                .replace("\"emitted_sequence\": 0", "\"emitted_sequence\": 17")
                .replace("\"emitted_block_height\": 0", "\"emitted_block_height\": 501")
                .bytes()
        )

        assertEquals("committed", response.submissionStatus)
        assertNull(response.transactionHash)
        assertEquals(17L, response.receipt.emittedSequence)
        assertEquals(501L, response.receipt.emittedBlockHeight)
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
                  "continue_cursor": null
                }
            """.trimIndent().bytes()
        )

        assertEquals(1, response.receipts.size)
        assertEquals(3L, response.total)
        assertEquals(1L, response.returnedItems)
        assertEquals(2L, response.remainingItems)
        assertEquals("exact", response.countMode)
        assertNull(response.continueCursor)
        assertEquals("2026.1", response.receipts.single().serviceVersion)
    }

    @Test
    fun boundedReceiptListLeavesTotalAbsent() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            """
                {
                  "schema_version": 1,
                  "receipts": [],
                  "returned_items": 0,
                  "remaining_items": 0,
                  "has_more": false,
                  "count_mode": "bounded"
                }
            """.trimIndent().bytes()
        )

        assertNull(response.total)
        assertFalse(response.hasMore)
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
                    .replace("\"submission_status\": \"submitted\"", "\"submission_status\": \"pending\"")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"transaction_hash\": \"transaction-hash\"", "\"transaction_hash\": null")
                    .bytes()
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"submission_status\": \"submitted\"", "\"submission_status\": \"committed\"")
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
                executeResponseJson().replace(PUBLIC_KEY_BASE64, "not-base64").bytes()
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
    fun rejectsMismatchedResponseOutputArtifact() {
        val canonical = executeResponseJson()
        val target = "\"output-manifest\""
        val responseOutput = canonical.lastIndexOf(target)
        val mismatched = canonical.substring(0, responseOutput) +
            "\"different-output-manifest\"" +
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
    fun rejectsBlankReceiptIdentityFields() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"receipt_id\": \"receipt-1\"", "\"receipt_id\": \"   \"")
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
              "count_mode": "exact"
            }
        """.trimIndent()

    private fun executeResponseJson(): String =
        """
            {
              "schema_version": 1,
              "status": {
                "status": "finalized",
                "service_name": "portal"
              },
              "submission_status": "submitted",
              "transaction_hash": "transaction-hash",
              "receipt": ${receiptJson()},
              "output_artifact": ${outputArtifactJson()}
            }
        """.trimIndent()

    private fun receiptJson(): String =
        """
            {
              "schema_version": 1,
              "network_id": "$NETWORK_ID",
              "receipt_id": "receipt-1",
              "service_name": "portal",
              "service_version": "2026.1",
              "model_id": "upload-1",
              "weight_version": "v1",
              "runtime_version": "soracloud.quantized-cpu.v1",
              "model_manifest_digest": "model-manifest",
              "model_bundle_root": "bundle-root",
              "policy_id": "policy-1",
              "decryption_request_id": "decrypt-upload-1",
              "attesting_validator": {
                "lane_id": 0,
                "validator_account_id": "validator@public",
                "peer_id": "peer-1"
              },
              "input_artifact": {
                "schema_version": 1,
                "sorafs_manifest_digest": "input-manifest",
                "sorafs_root_cid": $ROOT_CID_JSON,
                "artifact_hash": "input-artifact",
                "ciphertext_bytes": 64,
                "artifact_role": "input"
              },
              "output_artifact": ${outputArtifactJson()},
              "input_commitment": "input-commitment",
              "output_commitment": "output-commitment",
              "output_recipient": {
                "schema_version": 1,
                "key_id": "recipient-key",
                "key_version": 1,
                "kem": {"kem": "X25519HkdfSha256", "value": null},
                "aead": {"aead": "Aes256Gcm", "value": null},
                "public_key_bytes": "$PUBLIC_KEY_BASE64",
                "public_key_fingerprint": "recipient-fingerprint"
              },
              "request_commitment": "request-commitment",
              "result_commitment": "result-commitment",
              "emitted_sequence": 0,
              "emitted_block_height": 0
            }
        """.trimIndent()

    private fun outputArtifactJson(): String =
        """
            {
              "schema_version": 1,
              "sorafs_manifest_digest": "output-manifest",
              "sorafs_root_cid": $ROOT_CID_JSON,
              "artifact_hash": "output-artifact",
              "ciphertext_bytes": 96,
              "artifact_role": "output"
            }
        """.trimIndent()

    private fun String.bytes(): ByteArray = toByteArray(StandardCharsets.UTF_8)

    private companion object {
        const val NETWORK_ID = "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
        const val ROOT_CID_JSON = "[1, 113, 31, 32, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32]"
        const val ZERO_DIGEST_ROOT_CID_JSON = "[1, 113, 31, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]"
        const val PUBLIC_KEY_BASE64 = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
    }
}
