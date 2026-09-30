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
import org.hyperledger.iroha.sdk.alias.AccountFaucetClaimV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPolicyV1
import org.hyperledger.iroha.sdk.alias.AccountFaucetPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingCurrentStateV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanReceiptV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPlanRequestV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingPreparedTransactionV1
import org.hyperledger.iroha.sdk.alias.AccountOnboardingProofRequiredPrepareResponseV1
import org.hyperledger.iroha.sdk.alias.PreparedTransactionSubmitResponseV1
import org.hyperledger.iroha.sdk.alias.PreparedOperationBindingV1
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.core.util.HashLiteral
import org.hyperledger.iroha.sdk.tx.SignedTransaction

class SumeragiHttpTransportContractTest {
    @Test
    fun `status uses one exact bounded JSON GET and returns the authoritative model`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val executor = FixedResponseExecutor(jsonResponse(payload))
        val transport = transport(executor)

        val status = transport.getSumeragiStatus().join()

        assertEquals(1, status.protocolVersion)
        assertEquals("https://torii.example/api/v1/sumeragi/status", executor.request.uri.toString())
        assertEquals("GET", executor.request.method)
        assertTrue(executor.request.body.isEmpty())
        assertEquals(listOf("application/json"), executor.request.headers["Accept"])
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.request.replayPolicy)
        assertTrue(executor.request.headers.containsKey(OperatorRequestSigner.HEADER_SIGNATURE))
        assertEquals(1L * 1024L * 1024L, executor.request.maximumResponseBytes)
    }

    @Test
    fun `lanes use one exact bounded operator JSON GET and parse the Rust lane corpus`() {
        val payload = org.hyperledger.iroha.sdk.consensus.NativeLaneFixtures.json("mixed_lanes")
            .toByteArray(StandardCharsets.UTF_8)
        val executor = FixedResponseExecutor(jsonResponse(payload))

        val lanes = transport(executor).getSumeragiLanes().join()

        assertEquals(3, lanes.size)
        assertEquals(BigInteger.ONE, lanes[0].record.lane)
        assertEquals(1, lanes[0].instance?.protocolVersion)
        assertEquals(null, lanes[1].instance)
        assertEquals("https://torii.example/api/v1/sumeragi/lanes", executor.request.uri.toString())
        assertEquals("GET", executor.request.method)
        assertTrue(executor.request.body.isEmpty())
        assertEquals(listOf("application/json"), executor.request.headers["Accept"])
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.request.replayPolicy)
        assertTrue(executor.request.headers.containsKey(OperatorRequestSigner.HEADER_SIGNATURE))
        assertEquals(16L * 1024L * 1024L, executor.request.maximumResponseBytes)

        assertFails("the lane route must reject a status-shaped payload") {
            transport(FixedResponseExecutor(jsonResponse(statusJson().toByteArray()))).getSumeragiLanes().join()
        }
        val textResponse = TransportResponse.builder()
            .setStatusCode(200)
            .setBody(payload)
            .setHeaders(mapOf("Content-Type" to listOf("text/plain")))
            .build()
        assertFails { transport(FixedResponseExecutor(textResponse)).getSumeragiLanes().join() }
    }

    @Test
    fun `status accepts parameters and reject malformed or ambiguous JSON content types`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val diagnosticsPayload = diagnosticsJson().toByteArray(StandardCharsets.UTF_8)
        assertFails("status endpoint must reject a diagnostics-shaped payload") {
            transport(
                FixedResponseExecutor(
                    jsonResponse(diagnosticsPayload),
                ),
            ).getSumeragiStatus().join()
        }
        val validHeaders = listOf(
            mapOf("Content-Type" to listOf("Application/JSON; charset=utf-8")),
            mapOf(
                "content-type" to
                    listOf("application/json; charset=\"UTF-8\"; profile=exact"),
            ),
        )
        validHeaders.forEach { headers ->
            val statusResponse = TransportResponse.builder()
                .setStatusCode(200)
                .setBody(payload)
                .setHeaders(headers)
                .build()
            assertEquals(
                1,
                transport(FixedResponseExecutor(statusResponse)).getSumeragiStatus().join().protocolVersion,
            )


        }
        val invalidHeaders = listOf(
            emptyMap(),
            mapOf("Content-Type" to listOf("application/json", "application/json")),
            mapOf("Content-Type" to listOf("application/problem+json")),
            mapOf("Content-Type" to listOf("application/json, text/plain")),
            mapOf("Content-Type" to listOf("application/json;")),
            mapOf("Content-Type" to listOf("application/json; charset")),
            mapOf("Content-Type" to listOf("application/json; profile=\"a,b\"")),
        )
        invalidHeaders.forEach { headers ->
            val statusResponse = TransportResponse.builder()
                .setStatusCode(200)
                .setBody(payload)
                .setHeaders(headers)
                .build()
            assertFails {
                transport(FixedResponseExecutor(statusResponse)).getSumeragiStatus().join()
            }

        }
    }

    @Test
    fun `status requires exact 200 and a JSON media type`() {
        val payload = statusJson().toByteArray(StandardCharsets.UTF_8)
        val invalidResponses = listOf(
            TransportResponse.builder()
                .setStatusCode(201)
                .setBody(payload)
                .setHeaders(mapOf("Content-Type" to listOf("application/json")))
                .build(),
            TransportResponse.builder()
                .setStatusCode(204)
                .setBody(ByteArray(0))
                .setHeaders(mapOf("Content-Type" to listOf("application/json")))
                .build(),
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(payload)
                .setHeaders(mapOf("Content-Type" to listOf("text/html")))
                .build(),
        )
        invalidResponses.forEach { response ->
            val executor = FixedResponseExecutor(response)
            assertFails { transport(executor).getSumeragiStatus().join() }
            assertTrue(executor.hasRequest())
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
            override fun planSponsoredAccountOnboarding(
                request: AccountOnboardingPlanRequestV1,
                onboardingToken: String,
                expectedAuthority: String,
                expectedNetworkId: NetworkId,
            ): CompletableFuture<AccountOnboardingPlanReceiptV1> =
                throw AssertionError("account onboarding is not used by this interface-default test")

            override fun prepareSponsoredAccountOnboarding(
                request: AccountOnboardingPlanRequestV1,
                receipt: AccountOnboardingPlanReceiptV1,
                binding: PreparedOperationBindingV1,
                feePayment: FeePaymentIntent,
                onboardingToken: String,
                expectedAuthority: String,
                expectedNetworkId: NetworkId,
            ): CompletableFuture<AccountOnboardingPrepareResponseV1> =
                throw AssertionError("account onboarding is not used by this interface-default test")

            override fun verifyAccountOnboardingCurrentState(
                proofRequired: AccountOnboardingProofRequiredPrepareResponseV1,
                request: AccountOnboardingPlanRequestV1,
                receipt: AccountOnboardingPlanReceiptV1,
                binding: PreparedOperationBindingV1,
                expectedAuthority: String,
                expectedNetworkId: NetworkId,
                canonicalAuth: ToriiCanonicalRequestAuth,
            ): CompletableFuture<AccountOnboardingCurrentStateV1> =
                throw AssertionError("account onboarding is not used by this interface-default test")

            override fun submitPreparedAccountOnboarding(
                request: AccountOnboardingPlanRequestV1,
                prepared: AccountOnboardingPreparedTransactionV1,
                expectedFeePayment: FeePaymentIntent,
                onboardingToken: String,
                expectedAuthority: String,
                expectedNetworkId: NetworkId,
            ): CompletableFuture<PreparedTransactionSubmitResponseV1> =
                throw AssertionError("account onboarding is not used by this interface-default test")

            override fun prepareAccountFaucetTransaction(
                claim: AccountFaucetClaimV1,
                binding: PreparedOperationBindingV1,
                feePayment: FeePaymentIntent,
                policy: AccountFaucetPolicyV1,
                expectedNetworkId: NetworkId,
            ): CompletableFuture<AccountFaucetPreparedTransactionV1> =
                throw AssertionError("account faucet is not used by this interface-default test")

            override fun submitPreparedAccountFaucetTransaction(
                prepared: AccountFaucetPreparedTransactionV1,
                expectedFeePayment: FeePaymentIntent,
                policy: AccountFaucetPolicyV1,
                expectedNetworkId: NetworkId,
            ): CompletableFuture<PreparedTransactionSubmitResponseV1> =
                throw AssertionError("account faucet is not used by this interface-default test")

            override fun submitTransaction(
                transaction: SignedTransaction,
            ): CompletableFuture<ClientResponse> = CompletableFuture.completedFuture(
                ClientResponse(202, ByteArray(0), "accepted"),
            )
        }
        assertFails { defaultClient.getSumeragiStatus().join() }
    }

    @Test
    fun `operator reads reject missing and fallback authentication before dispatch`() {
        val executor = FixedResponseExecutor(jsonResponse(statusJson().toByteArray()))
        val missing = HttpClientTransport(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .build(),
        )
        assertFails { missing.getSumeragiStatus() }
        assertTrue(!executor.hasRequest())

        val fallback = HttpClientTransport(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setOperatorSigningContext(operatorContext())
                .putDefaultHeader("Authorization", "Bearer retired")
                .build(),
        )
        assertFails { fallback.getSumeragiStatus() }
        assertTrue(!executor.hasRequest())
    }

    private fun transport(executor: HttpTransportExecutor): HttpClientTransport =
        HttpClientTransport(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setOperatorSigningContext(operatorContext())
                .build(),
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
        private var recordedRequest: TransportRequest? = null

        val request: TransportRequest get() = checkNotNull(recordedRequest)

        fun hasRequest(): Boolean = recordedRequest != null

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            this.recordedRequest = request
            return CompletableFuture.completedFuture(response)
        }
    }

    companion object {
        private fun operatorContext(): OperatorSigningContext {
            val bytes = ByteArray(32) { 0x5a }
            bytes[31] = (bytes[31].toInt() or 1).toByte()
            return OperatorSigningContext(
                NetworkId.fromBytes(bytes),
                "ed0120${"66".repeat(32)}",
                OperatorRequestSignatureProvider { ByteArray(64) { 0x55 } },
            )
        }

        private fun hash(seed: Int): String =
            HashLiteral.canonicalize(ByteArray(32) { seed.toByte() })

        private fun diagnosticsJson(): String = """
            {
              "tx_queue_depth": 0,
              "tx_queue_capacity": 1,
              "tx_queue_retained_bytes": 0,
              "tx_queue_max_retained_bytes": 1,
              "tx_queue_saturated": false,
              "tx_queue_saturated_by_count": false,
              "tx_queue_saturated_by_bytes": false,
              "tx_queue_saturated_by_age": false,
              "tx_queue_oldest_queued_age_ms": 0,
              "lane_governance_sealed_total": 0,
              "lane_governance_sealed_aliases": [],
              "lane_governance": []
            }
        """.trimIndent()

        private fun statusJson(): String = org.hyperledger.iroha.sdk.consensus.NativeStatusFixtures.json("observer")
    }
}
