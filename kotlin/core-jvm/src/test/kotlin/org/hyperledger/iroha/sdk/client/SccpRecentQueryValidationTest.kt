package org.hyperledger.iroha.sdk.client

import java.net.URI
import java.nio.charset.StandardCharsets
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CompletionException
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class SccpRecentQueryValidationTest {

    @Test
    fun invalidWindowsAreRejectedBeforeHttpExecution() {
        val executor = CountingExecutor()
        val transport = transport(executor)

        for (from in listOf(Long.MIN_VALUE, -1L, 0L)) {
            assertFailsWith<IllegalArgumentException>("from=$from") {
                transport.getSccpRecentMessages(from, 1)
            }
        }
        for (limit in listOf(Int.MIN_VALUE, -1, 0, 51, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException>("limit=$limit") {
                transport.getSccpRecentMessages(1, limit)
            }
        }

        assertEquals(0, executor.requests.size, "invalid queries must not reach HTTP execution")
    }

    @Test
    fun exactWindowBoundariesReachHttpWithCanonicalQuery() {
        val executor = CountingExecutor()
        val transport = transport(executor)

        transport.getSccpRecentMessages(1, 1).join()
        transport.getSccpRecentMessages(Long.MAX_VALUE, 50).join()

        assertEquals(2, executor.requests.size)
        assertEquals("from=1&limit=1", executor.requests[0].uri.rawQuery)
        assertEquals(
            "from=${Long.MAX_VALUE}&limit=50",
            executor.requests[1].uri.rawQuery,
        )
    }

    @Test
    fun sccpEndpointsCarryNarrowResponseLimits() {
        val executor = CountingExecutor()
        val transport = transport(executor)
        val messageId = "11".repeat(32)

        transport.getSccpCapabilities()
        transport.getSccpRegistry()
        transport.getSccpMessageBundle(messageId)
        transport.getSccpProofRequest(messageId)
        transport.getSccpRecentMessages(1, 1)

        assertEquals(
            listOf(
                64L * 1024L,
                64L * 1024L * 1024L,
                64L * 1024L * 1024L,
                64L * 1024L * 1024L,
                8L * 1024L * 1024L,
            ),
            executor.requests.map { it.maximumResponseBytes },
        )
    }

    @Test
    fun sccpJsonRequiresExact200AndUnambiguousCanonicalContentType() {
        val invalidResponses = listOf(
            response(201, listOf("application/json")),
            response(204, listOf("application/json"), ByteArray(0)),
            response(200, listOf("text/html")),
            response(200, null),
            response(200, listOf("application/json", "application/json")),
            response(200, listOf("application/json; charset=utf-8")),
        )
        for (response in invalidResponses) {
            val executor = CountingExecutor(response)
            val failure = assertFailsWith<CompletionException> {
                transport(executor).getSccpRecentMessages(1, 1).join()
            }
            assertTrue(failure.cause is RuntimeException)
            assertEquals(1, executor.requests.size)
        }
    }

    private fun transport(executor: CountingExecutor): HttpClientTransport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example"))
                .build(),
        )

    private class CountingExecutor(
        private val response: TransportResponse = response(
            200,
            listOf("application/json"),
        ),
    ) : HttpTransportExecutor {
        val requests = mutableListOf<TransportRequest>()

        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
            requests += request
            return CompletableFuture.completedFuture(response)
        }
    }

    companion object {
        private fun response(
            status: Int,
            contentTypes: List<String>?,
            body: ByteArray = "{\"items\":[]}".toByteArray(StandardCharsets.UTF_8),
        ): TransportResponse {
            val builder = TransportResponse.builder()
                .setStatusCode(status)
                .setBody(body)
            contentTypes?.forEach { builder.addHeader("Content-Type", it) }
            return builder.build()
        }
    }
}
