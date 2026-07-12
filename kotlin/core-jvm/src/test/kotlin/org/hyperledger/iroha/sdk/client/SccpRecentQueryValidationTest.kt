package org.hyperledger.iroha.sdk.client

import java.math.BigInteger
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

        for (from in listOf(
            BigInteger.valueOf(Long.MIN_VALUE),
            BigInteger.valueOf(-1),
            BigInteger.ZERO,
            BigInteger.ONE.shiftLeft(64),
        )) {
            assertFailsWith<IllegalArgumentException>("from=$from") {
                transport.getSccpRecentMessages(from, null, 1)
            }
        }
        for (limit in listOf(Int.MIN_VALUE, -1, 0, 51, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException>("limit=$limit") {
                transport.getSccpRecentMessages(BigInteger.ONE, null, limit)
            }
        }
        for (afterIndex in listOf(Int.MIN_VALUE, -1, 512, Int.MAX_VALUE)) {
            assertFailsWith<IllegalArgumentException>("afterIndex=$afterIndex") {
                transport.getSccpRecentMessages(BigInteger.ONE, afterIndex, 1)
            }
        }
        assertFailsWith<IllegalArgumentException>("unpaired afterIndex") {
            transport.getSccpRecentMessages(null, 0, 1)
        }
        assertFailsWith<IllegalArgumentException> { SccpRecentCursor(BigInteger.ZERO, 0) }
        assertFailsWith<IllegalArgumentException> {
            SccpRecentCursor(BigInteger.ONE.shiftLeft(64), 0)
        }
        assertFailsWith<IllegalArgumentException> { SccpRecentCursor(BigInteger.ONE, 512) }

        assertEquals(0, executor.requests.size, "invalid queries must not reach HTTP execution")
    }

    @Test
    fun exactWindowBoundariesReachHttpWithCanonicalQuery() {
        val executor = CountingExecutor()
        val transport = transport(executor)

        val maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)
        transport.getSccpRecentMessages(BigInteger.ONE, null, 1).join()
        transport.getSccpRecentMessages(maxU64, 0, 50).join()
        transport.getSccpRecentMessages(SccpRecentCursor(BigInteger.valueOf(7), 511), 1).join()

        assertEquals(3, executor.requests.size)
        assertEquals("from=1&limit=1", executor.requests[0].uri.rawQuery)
        assertEquals(
            "from=$maxU64&after_index=0&limit=50",
            executor.requests[1].uri.rawQuery,
        )
        assertEquals("from=7&after_index=511&limit=1", executor.requests[2].uri.rawQuery)
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
        transport.getSccpRecentMessages(BigInteger.ONE, null, 1)

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
                transport(executor).getSccpRecentMessages(BigInteger.ONE, null, 1).join()
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
