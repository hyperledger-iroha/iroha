// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNotNull
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.core.model.NetworkId
import org.hyperledger.iroha.sdk.norito.CRC64
import org.hyperledger.iroha.sdk.norito.NoritoHeader

class HttpClientTransportGovernanceTest {
    private val networkId = NetworkId.parse(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
    )

    @Test
    fun getGovernanceContractParsesResponse() {
        val contractAddress = "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = """
                {
                  "found": true,
                  "contract_address": "$contractAddress",
                  "dataspace": "router",
                  "code_hash_hex": "${"77".repeat(32)}"
                }
            """.trimIndent().toByteArray(StandardCharsets.UTF_8),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )

        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_100L,
            "governance-read",
        )
        val response = transport.getGovernanceContract(contractAddress, auth).join()

        assertTrue(response.found)
        assertEquals(contractAddress, response.contractAddress)
        assertEquals("router", response.dataspace)
        assertEquals("77".repeat(32), response.codeHashHex)

        val request = assertNotNull(executor.lastRequest)
        assertEquals("GET", request.method)
        assertEquals(
            "https://torii.example/api/v1/gov/contracts/$contractAddress",
            request.uri.toString(),
        )
        assertTrue(request.body.isEmpty())
    }

    @Test
    fun parliamentCastingProofPostIsExactAuthenticatedBoundedAndOneShot() {
        val responsePayload = byteArrayOf(2, 1, 0, 1)
        val responseHeader = NoritoHeader(
            decodeHex(ParliamentApiV1.TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_HASH_HEX),
            responsePayload.size,
            CRC64.compute(responsePayload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        )
        val responseFrame = responseHeader.encode() + responsePayload
        val executor = StubResponseExecutor(
            statusCode = 200,
            body = responseFrame,
            headers = mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Content-Length" to listOf(responseFrame.size.toString()),
            ),
        )
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_100L,
            "parliament-casting-proof",
        )
        val ballotId = "33".repeat(32)
        val response = transport.getParliamentTimedOvnCastingProofPageV1(
            ballotId,
            BigInteger.valueOf(17),
            auth,
        ).join()

        assertContentEquals(responseFrame, response.canonicalNorito())
        val request = assertNotNull(executor.lastRequest)
        assertEquals("POST", request.method)
        assertEquals(
            "/api/v1/gov/parliament/ballots/$ballotId/casting-proof",
            request.uri.rawPath,
        )
        assertContentEquals(
            ParliamentApiV1.timedOvnCastingProofRequestNorito(17),
            request.body,
        )
        assertEquals(listOf("application/x-norito"), request.headers["Content-Type"])
        assertEquals(listOf("application/x-norito"), request.headers["Accept"])
        assertEquals(listOf("identity"), request.headers["Accept-Encoding"])
        assertEquals(
            ParliamentApiV1.MAX_TIMED_OVN_CASTING_PROOF_RESPONSE_BYTES.toLong(),
            request.maximumResponseBytes,
        )
        assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy)
        assertEquals(1, executor.requestCount)
        assertTrue(request.headers.keys.any { it.equals("X-Iroha-Signature", true) })

        val encodedExecutor = StubResponseExecutor(
            statusCode = 200,
            body = responseFrame,
            headers = mapOf(
                "Content-Type" to listOf("application/x-norito"),
                "Content-Encoding" to listOf("gzip"),
            ),
        )
        val encodedTransport = HttpClientTransport.withExecutor(
            encodedExecutor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )
        assertFailsWith<CompletionException> {
            encodedTransport.requestParliamentTimedOvnCastingProofV1(
                ballotId,
                17,
                auth,
            ).join()
        }
        assertEquals(1, encodedExecutor.requestCount)
    }

    @Test
    fun parliamentCastingContextGetIsAuthenticatedAndBounded() {
        val executor = StubResponseExecutor(
            statusCode = 404,
            body = ByteArray(0),
        )
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_700_000_000_100L,
            "parliament-casting-context",
        )
        val ballotId = "44".repeat(32)

        assertFailsWith<CompletionException> {
            transport.getParliamentTimedOvnCastingContextV1(ballotId, auth).join()
        }
        val request = assertNotNull(executor.lastRequest)
        assertEquals("GET", request.method)
        assertEquals(
            "/api/v1/gov/parliament/ballots/$ballotId/casting-context",
            request.uri.rawPath,
        )
        assertContentEquals(ByteArray(0), request.body)
        assertEquals(ParliamentApiV1.MAX_STATE_BYTES.toLong(), request.maximumResponseBytes)
        assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy)
        assertTrue(request.headers.keys.any { it.equals("X-Iroha-Signature", true) })
        assertEquals(1, executor.requestCount)
    }

    @Test
    fun parliamentCastingProofPagingDurablyAdvancesStaleAnchorBeyondSixtyThreeHeights() {
        val responseFrame = castingProofResponseFrame()
        val executor = SequenceResponseExecutor(responseFrame)
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth("alice@universal", keyPair.private)
        val firstContext = ByteArray(32) { 0x11 }
        val secondContext = ByteArray(32) { 0x22 }
        val terminalContext = ByteArray(32) { 0x33 }
        val verifierCalls = AtomicInteger()
        val persisted = mutableListOf<ParliamentTimedOvnCastingProofPageVerificationV1>()
        val firstPersistence = CompletableFuture<Void>()

        val future = transport.requestParliamentTimedOvnCastingProofUntilTerminalV1(
            "55".repeat(32),
            7,
            firstContext,
            auth,
            ParliamentTimedOvnCastingProofPageVerifierV1 { _, height, context ->
                when (verifierCalls.getAndIncrement()) {
                    0 -> {
                        assertEquals(BigInteger.valueOf(7), height)
                        assertContentEquals(firstContext, context)
                        ParliamentTimedOvnCastingProofPageVerificationV1(
                            BigInteger.valueOf(70),
                            secondContext,
                            true,
                        )
                    }
                    1 -> {
                        assertEquals(BigInteger.valueOf(70), height)
                        assertContentEquals(secondContext, context)
                        ParliamentTimedOvnCastingProofPageVerificationV1(
                            BigInteger.valueOf(75),
                            terminalContext,
                            false,
                        )
                    }
                    else -> error("unexpected casting-proof page")
                }
            },
            ParliamentTimedOvnCastingCheckpointPersisterV1 { verification ->
                persisted += verification
                if (persisted.size == 1) firstPersistence
                else CompletableFuture.completedFuture(null)
            },
        )

        assertEquals(1, executor.requests.size)
        assertTrue(!future.isDone)
        firstPersistence.complete(null)
        val terminal = future.join()

        assertEquals(2, executor.requests.size)
        assertEquals(2, persisted.size)
        assertEquals(2, terminal.verifiedPageCount)
        assertEquals(BigInteger.valueOf(70), terminal.verificationAnchorHeight)
        assertContentEquals(secondContext, terminal.verificationAnchorContextId())
        assertEquals(BigInteger.valueOf(75), terminal.verification.evaluatedBlockHeight)
        assertTrue(!terminal.verification.moreAvailable)
        assertContentEquals(
            ParliamentApiV1.timedOvnCastingProofRequestNorito(7),
            executor.requests[0].body,
        )
        assertContentEquals(
            ParliamentApiV1.timedOvnCastingProofRequestNorito(70),
            executor.requests[1].body,
        )
    }

    @Test
    fun parliamentCastingProofPagingRejectsNativeAdvancePastPageBound() {
        val executor = SequenceResponseExecutor(castingProofResponseFrame())
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .setLocalSigningContext(LocalSigningContext(networkId))
                .build(),
        )
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        var persistCalls = 0
        val failure = assertFailsWith<CompletionException> {
            transport.requestParliamentTimedOvnCastingProofUntilTerminalV1(
                "66".repeat(32),
                7,
                ByteArray(32) { 0x11 },
                ToriiCanonicalRequestAuth("alice@universal", keyPair.private),
                ParliamentTimedOvnCastingProofPageVerifierV1 { _, _, _ ->
                    ParliamentTimedOvnCastingProofPageVerificationV1(
                        BigInteger.valueOf(71),
                        ByteArray(32) { 0x22 },
                        true,
                    )
                },
                ParliamentTimedOvnCastingCheckpointPersisterV1 {
                    persistCalls += 1
                    CompletableFuture.completedFuture(null)
                },
            ).join()
        }
        assertTrue(failure.cause is IllegalArgumentException)
        assertEquals(0, persistCalls)
        assertEquals(1, executor.requests.size)
    }

    private fun castingProofResponseFrame(): ByteArray {
        val responsePayload = byteArrayOf(2, 1, 0, 1)
        return NoritoHeader(
            decodeHex(ParliamentApiV1.TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_HASH_HEX),
            responsePayload.size,
            CRC64.compute(responsePayload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE,
        ).encode() + responsePayload
    }

    private fun decodeHex(value: String): ByteArray = ByteArray(value.length / 2) { index ->
        value.substring(index * 2, index * 2 + 2).toInt(16).toByte()
    }

    private class StubResponseExecutor(
        private val statusCode: Int,
        private val body: ByteArray,
        private val headers: Map<String, List<String>> = emptyMap(),
    ) : HttpTransportExecutor {
        var lastRequest: TransportRequest? = null
        var requestCount: Int = 0

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            requestCount += 1
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(
                    TransportResponse.builder()
                        .setStatusCode(200)
                        .setBody(
                            (
                                "{\"data_model_version\":4,\"signed_transaction_schema_hash_hex\":" +
                                    "\"7ab5ff9c572efb316deac478f19209c5\"}"
                                ).toByteArray(StandardCharsets.UTF_8),
                        )
                        .build(),
                )
            }
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(statusCode)
                    .setBody(body)
                    .setHeaders(headers)
                    .build(),
            )
        }
    }

    private class SequenceResponseExecutor(
        private val body: ByteArray,
    ) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests += request
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(200)
                    .setBody(body)
                    .setHeaders(
                        mapOf(
                            "Content-Type" to listOf("application/x-norito"),
                            "Content-Length" to listOf(body.size.toString()),
                        ),
                    )
                    .build(),
            )
        }
    }
}
