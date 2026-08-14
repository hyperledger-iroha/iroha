package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyPairGenerator
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.testing.TestNetworkIds

/** Focused transaction-submission wire and capability-probe contract tests. */
class HttpClientTransportSubmissionContractTest {
    @Test
    fun submitTransactionJsonUsesJsonIngressWithConfiguredAccept() {
        val executor = CapturingExecutor()
        val transport = HttpClientTransport.withExecutor(
            executor = executor,
            config = ClientConfig.builder()
                .setBaseUri(URI.create("https://127.0.0.1:8080"))
                .setWireFormatPreference(WireFormatPreference.JSON_PREFERRED)
                .build(),
        )
        val body = """{"version":1,"content":{}}""".toByteArray(StandardCharsets.UTF_8)

        val response = transport.submitTransactionJson(body).join()

        assertEquals(202, response.statusCode)
        val request = executor.lastRequest
        assertEquals("POST", request.method)
        assertEquals("https://127.0.0.1:8080/v1/pipeline/transactions", request.uri.toString())
        assertEquals("application/json", request.headers["Content-Type"]?.first())
        assertEquals(WireFormatPreference.JSON_PREFERRED.acceptHeader(), request.headers["Accept"]?.first())
        assertTrue(body.contentEquals(request.body))
        assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy)
    }

    @Test
    fun replayPolicyIsSafeOnlyForUnsignedBodylessReads() {
        val unsignedRead = TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/v1/status"))
            .build()
        val signedQuery = TransportRequest.builder()
            .setMethod("GET")
            .setUri(URI.create("https://127.0.0.1:8080/v1/query"))
            .addHeader("X-Iroha-Signature", "signature")
            .addHeader("X-Iroha-Nonce", "nonce")
            .build()

        assertEquals(RequestReplayPolicy.RETRY_SAFE, unsignedRead.replayPolicy)
        assertEquals(RequestReplayPolicy.ONE_SHOT, signedQuery.replayPolicy)
    }

    @Test
    fun canonicalAuthRedirectStatusAndNetworkFailuresAreOneShot() {
        val keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair()
        val auth = ToriiCanonicalRequestAuth(
            "alice@universal",
            keyPair.private,
            1_717_171_717_000L,
            "canonical-one-shot-nonce",
        )
        for (status in listOf(307, 308, 503)) {
            assertCanonicalAliasFailsOnce(auth, OutcomeExecutor(status = status))
        }
        assertCanonicalAliasFailsOnce(
            auth,
            OutcomeExecutor(failure = RuntimeException("ambiguous network failure")),
        )
    }

    private fun assertCanonicalAliasFailsOnce(
        auth: ToriiCanonicalRequestAuth,
        executor: OutcomeExecutor,
    ) {
        val transport = HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://127.0.0.1:8080"))
                .setLocalSigningContext(LocalSigningContext(TestNetworkIds.canonical()))
                .build(),
        )

        assertFailsWith<CompletionException> {
            transport.resolveAccountAlias("merchant@private", auth).join()
        }
        assertEquals(1, executor.callCount)
        assertEquals(RequestReplayPolicy.ONE_SHOT, executor.lastRequest.replayPolicy)
    }

    private class CapturingExecutor : HttpTransportExecutor {
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            lastRequest = request
            if (request.uri.path.endsWith("/v1/node/capabilities")) {
                return CompletableFuture.completedFuture(compatibleCapabilitiesResponse())
            }
            return CompletableFuture.completedFuture(
                TransportResponse.builder()
                    .setStatusCode(202)
                    .setBody(byteArrayOf())
                    .build(),
            )
        }
    }

    private class OutcomeExecutor(
        private val status: Int? = null,
        private val failure: RuntimeException? = null,
    ) : HttpTransportExecutor {
        var callCount = 0
        lateinit var lastRequest: TransportRequest

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            callCount += 1
            lastRequest = request
            val future = CompletableFuture<TransportResponse>()
            if (failure != null) {
                future.completeExceptionally(failure)
            } else {
                future.complete(
                    TransportResponse.builder()
                        .setStatusCode(requireNotNull(status))
                        .setBody(byteArrayOf())
                        .build(),
                )
            }
            return future
        }
    }

    private companion object {
        fun compatibleCapabilitiesResponse(): TransportResponse =
            TransportResponse.builder()
                .setStatusCode(200)
                .setBody(
                    (
                        "{\"data_model_version\":4,\"signed_transaction_schema_hash_hex\":" +
                            "\"7ab5ff9c572efb316deac478f19209c5\"}"
                        ).toByteArray(StandardCharsets.UTF_8),
                )
                .build()
    }
}
