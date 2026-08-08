package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

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
