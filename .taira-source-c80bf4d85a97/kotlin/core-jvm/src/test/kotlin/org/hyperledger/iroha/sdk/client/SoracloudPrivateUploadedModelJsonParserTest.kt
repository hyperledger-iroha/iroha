package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse

class SoracloudPrivateUploadedModelJsonParserTest {

    @Test
    fun parsesPrivateExecuteResponseAndReceiptInstruction() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
            executeResponseJson().toByteArray(StandardCharsets.UTF_8)
        )

        assertEquals(1L, response.schemaVersion)
        assertEquals("finalized", response.status["status"])
        assertEquals("receipt-1", response.receipt.receiptId)
        assertEquals("portal", response.receipt.serviceName)
        assertEquals("input", response.receipt.inputArtifact.artifactRole)
        assertEquals("output", response.receipt.outputArtifact.artifactRole)
        assertEquals(17L, response.receipt.emittedSequence)
        assertEquals(
            SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID,
            response.receiptInstruction().wireId,
        )
        assertEquals("0a0b0c", response.receiptInstruction().payloadHex)
    }

    @Test
    fun parsesPrivateReceiptListPaginationMetadata() {
        val response = SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
            """
                {
                  "schema_version": 1,
                  "receipts": [${receiptJson()}],
                  "total": 3,
                  "returned_items": 1,
                  "remaining_items": 2,
                  "has_more": true,
                  "count_mode": "exact",
                  "continue_cursor": null
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8)
        )

        assertEquals(1, response.receipts.size)
        assertEquals(3L, response.total)
        assertEquals(1L, response.returnedItems)
        assertEquals(2L, response.remainingItems)
        assertEquals("exact", response.countMode)
        assertEquals(null, response.continueCursor)
    }

    @Test
    fun rejectsMissingOrMalformedReceiptInstruction() {
        val missing = listOf(SoracloudTxInstruction("other", "0a"))
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(missing)
        }

        val malformed = listOf(
            SoracloudTxInstruction(
                SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID,
                "zz",
            )
        )
        assertFailsWith<IllegalArgumentException> {
            SoracloudPrivateUploadedModelJsonParser.privateUploadedModelReceiptInstruction(malformed)
        }
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
            """.trimIndent().toByteArray(StandardCharsets.UTF_8)
        )

        assertEquals(null, response.total)
        assertFalse(response.hasMore)
    }

    @Test
    fun rejectsNegativeReceiptPaginationMetadata() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(total = "-1").toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(returnedItems = "-1").toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseReceiptList(
                receiptListJson(remainingItems = "-1").toByteArray(StandardCharsets.UTF_8)
            )
        }
    }

    @Test
    fun rejectsNegativeReceiptArtifactAndSequenceFields() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"ciphertext_bytes\": 64", "\"ciphertext_bytes\": -1")
                    .toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"emitted_sequence\": 17", "\"emitted_sequence\": -1")
                    .toByteArray(StandardCharsets.UTF_8)
            )
        }
    }

    @Test
    fun rejectsBlankReceiptIdentityFields() {
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"receipt_id\": \"receipt-1\"", "\"receipt_id\": \"   \"")
                    .toByteArray(StandardCharsets.UTF_8)
            )
        }
        assertFailsWith<IllegalStateException> {
            SoracloudPrivateUploadedModelJsonParser.parseExecuteResponse(
                executeResponseJson()
                    .replace("\"policy_id\": \"policy-1\"", "\"policy_id\": \"\"")
                    .toByteArray(StandardCharsets.UTF_8)
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
              "receipt": ${receiptJson()},
              "tx_instructions": [
                {
                  "wire_id": "${SoracloudPrivateUploadedModelJsonParser.PRIVATE_UPLOADED_MODEL_RECEIPT_WIRE_ID}",
                  "payload_hex": "0a0b0c"
                }
              ]
            }
        """.trimIndent()

    private fun receiptJson(): String =
        """
            {
              "schema_version": 1,
              "receipt_id": "receipt-1",
              "service_name": "portal",
              "model_id": "upload-1",
              "weight_version": "v1",
              "runtime_version": "soracloud.private.quantized_cpu.v1",
              "model_manifest_digest": "model-manifest",
              "model_bundle_root": "bundle-root",
              "policy_id": "policy-1",
              "input_artifact": {
                "schema_version": 1,
                "sorafs_manifest_digest": "input-manifest",
                "artifact_hash": "input-artifact",
                "ciphertext_bytes": 64,
                "artifact_role": "input"
              },
              "output_artifact": {
                "schema_version": 1,
                "sorafs_manifest_digest": "output-manifest",
                "artifact_hash": "output-artifact",
                "ciphertext_bytes": 96,
                "artifact_role": "output"
              },
              "input_commitment": "input-commitment",
              "output_commitment": "output-commitment",
              "request_commitment": "request-commitment",
              "result_commitment": "result-commitment",
              "emitted_sequence": 17
            }
        """.trimIndent()
}
