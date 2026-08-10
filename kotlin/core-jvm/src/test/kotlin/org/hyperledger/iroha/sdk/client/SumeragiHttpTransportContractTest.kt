// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFails
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.tx.SignedTransaction

class SumeragiHttpTransportContractTest {
    @Test
    fun `status uses one exact bounded JSON GET and returns the authoritative model`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val executor = FixedResponseExecutor(jsonResponse(payload))
        val transport = transport(executor)

        val status = transport.getSumeragiStatus().join()

        assertEquals(4, status.protocolVersion)
        assertEquals("https://torii.example/api/v1/sumeragi/status", executor.request.uri.toString())
        assertEquals("GET", executor.request.method)
        assertTrue(executor.request.body.isEmpty())
        assertEquals(listOf("application/json"), executor.request.headers["Accept"])
        assertEquals(1L * 1024L * 1024L, executor.request.maximumResponseBytes)
    }

    @Test
    fun `diagnostics uses one exact bounded JSON GET and returns the operational model`() {
        val payload = diagnosticsJson().toByteArray(StandardCharsets.UTF_8)
        val executor = FixedResponseExecutor(jsonResponse(payload))
        val transport = transport(executor)

        val diagnostics = transport.getSumeragiDiagnostics().join()

        assertEquals(BigInteger.ONE, diagnostics.txQueueCapacity)
        assertEquals(
            "https://torii.example/api/v1/sumeragi/diagnostics",
            executor.request.uri.toString(),
        )
        assertEquals("GET", executor.request.method)
        assertTrue(executor.request.body.isEmpty())
        assertEquals(listOf("application/json"), executor.request.headers["Accept"])
        assertEquals(16L * 1024L * 1024L, executor.request.maximumResponseBytes)
    }

    @Test
    fun `status and diagnostics reject missing parameterized or ambiguous JSON content types`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val invalidHeaders = listOf(
            emptyMap(),
            mapOf("Content-Type" to listOf("application/json; charset=utf-8")),
            mapOf("Content-Type" to listOf("application/json", "application/json")),
        )
        invalidHeaders.forEach { headers ->
            val response = TransportResponse.builder()
                .setStatusCode(200)
                .setBody(payload)
                .setHeaders(headers)
                .build()
            assertFails { transport(FixedResponseExecutor(response)).getSumeragiStatus().join() }
            assertFails { transport(FixedResponseExecutor(response)).getSumeragiDiagnostics().join() }
        }
    }

    @Test
    fun `status rejects noncanonical mismatched ambiguous and over-limit content lengths`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val invalidLengths = listOf(
            emptyList(),
            listOf("+${payload.size}"),
            listOf("0${payload.size}"),
            listOf((payload.size + 1).toString()),
            listOf(payload.size.toString(), payload.size.toString()),
        )
        invalidLengths.forEach { lengths ->
            val response = TransportResponse.builder()
                .setStatusCode(200)
                .setBody(payload)
                .setHeaders(
                    mapOf(
                        "Content-Type" to listOf("application/json"),
                        "Content-Length" to lengths,
                    ),
                )
                .build()
            assertFails { transport(FixedResponseExecutor(response)).getSumeragiStatus().join() }
        }

        val oversized = ByteArray(1 * 1024 * 1024 + 1) { ' '.code.toByte() }
        val response = jsonResponse(oversized)
        assertFails { transport(FixedResponseExecutor(response)).getSumeragiStatus().join() }
    }

    @Test
    fun `status rejects malformed UTF-8 and the interface default fails exceptionally`() {
        val malformed = byteArrayOf(0x7b, 0x22, 0xc3.toByte(), 0x28, 0x22, 0x7d)
        assertFails {
            transport(FixedResponseExecutor(jsonResponse(malformed))).getSumeragiStatus().join()
        }

        val defaultClient = object : IrohaClient {
            override fun submitTransaction(
                transaction: SignedTransaction,
            ): CompletableFuture<ClientResponse> = CompletableFuture.completedFuture(
                ClientResponse(202, ByteArray(0), "accepted"),
            )
        }
        assertFails { defaultClient.getSumeragiStatus().join() }
    }

    private fun transport(executor: HttpTransportExecutor): HttpClientTransport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("https://torii.example/api")).build(),
        )

    private fun jsonResponse(payload: ByteArray): TransportResponse =
        TransportResponse.builder()
            .setStatusCode(200)
            .setBody(payload)
            .setHeaders(
                mapOf(
                    "Content-Type" to listOf("application/json"),
                    "Content-Length" to listOf(payload.size.toString()),
                ),
            )
            .build()

    private class FixedResponseExecutor(
        private val response: TransportResponse,
    ) : HttpTransportExecutor {
        lateinit var request: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.request = request
            return CompletableFuture.completedFuture(response)
        }
    }

    companion object {
        private fun hash(seed: Int): String =
            HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })

        private fun diagnosticsJson(): String = """
            {
              "pipeline_execution": {
                "tx_vertices_total": 0,
                "tx_edges_total": 0,
                "overlay_count_total": 0,
                "overlay_instr_total": 0,
                "overlay_bytes_total": 0,
                "rbc_chunks_total": 0,
                "rbc_bytes_total": 0,
                "detached_prepared_total": 0,
                "detached_merged_total": 0,
                "detached_fallback_total": 0,
                "detached_fallback_fee_postprocessing_total": 0,
                "detached_fallback_user_executor_total": 0,
                "detached_fallback_durable_state_total": 0,
                "detached_fallback_unsupported_instruction_total": 0,
                "detached_fallback_rejected_eval_total": 0,
                "detached_fallback_overlay_error_total": 0,
                "quarantine_executed_total": 0
              },
              "tx_queue_depth": 0,
              "tx_queue_capacity": 1,
              "tx_queue_retained_bytes": 0,
              "tx_queue_max_retained_bytes": 1,
              "tx_queue_saturated": false,
              "tx_queue_saturated_by_count": false,
              "tx_queue_saturated_by_bytes": false,
              "tx_queue_saturated_by_age": false,
              "tx_queue_oldest_queued_age_ms": 0,
              "lane_commitments": [],
              "dataspace_commitments": [],
              "lane_settlement_commitments": [],
              "lane_relay_envelopes": [],
              "lane_payload_ownerships": [],
              "committed_lane_blocks": [],
              "lane_block_sessions": [],
              "lane_governance_sealed_total": 0,
              "lane_governance_sealed_aliases": [],
              "lane_governance": [],
              "native_amx_participant_applications": [],
              "autonomous_lane_executions": []
            }
        """.trimIndent()

        private fun statusJson(): String = """
            {
              "protocol_version": 4,
              "node_fingerprint": "${hash(0x11)}",
              "build_fingerprint": "${hash(0x12)}",
              "config_fingerprint": "${hash(0x13)}",
              "restart_required": false,
              "height_context_id": ["${hash(0x14)}"],
              "height": 1,
              "view": 0,
              "phase": {"phase": "awaiting_proposal", "details": null},
              "leader": 0,
              "locked_prepare_qc": null,
              "highest_prepare_qc": null,
              "last_timeout_certificate": null,
              "body_state": {"state": "missing", "details": null},
              "pending_persistence_id": null,
              "last_committed_height": 0,
              "last_committed_subject": null,
              "height_context": {
                "epoch": 0,
                "epoch_end_height": 1,
                "mode": {"mode": "permissioned", "details": null},
                "epoch_seed": "${"00".repeat(32)}",
                "validator_count": 4,
                "quorum": {"min_signers": 3, "total_power": 4}
              },
              "last_commit_qc": null,
              "liveness": {
                "generation": 0,
                "prepare_quorums": [],
                "commit_quorums": [],
                "timeout_quorums": [],
                "outbound_intents": [],
                "work": {
                  "candidate": {"stage": "idle", "details": null},
                  "body_recovery": {"stage": "idle", "details": null},
                  "body_store": {"stage": "idle", "details": null},
                  "validation": {"stage": "idle", "details": null},
                  "application": {"stage": "idle", "details": null},
                  "successor_height": {"stage": "idle", "details": null}
                },
                "queues": [],
                "last_progress": null,
                "no_progress_age_ms": 0,
                "blocker": null,
                "ignore_counts": []
              }
            }
        """.trimIndent()
    }
}
