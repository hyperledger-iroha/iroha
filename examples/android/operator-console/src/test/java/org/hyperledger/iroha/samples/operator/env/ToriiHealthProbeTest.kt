package org.hyperledger.iroha.samples.operator.env

import java.net.URI
import java.time.Duration
import java.util.concurrent.CompletableFuture
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue
import kotlinx.coroutines.runBlocking
import org.junit.Test
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse

class ToriiHealthProbeTest {
    @Test
    fun `probe uses SDK GET transport and exact five second timeout`() = runBlocking {
        val executor = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                assertEquals("GET", request.method)
                assertEquals(URI.create("https://torii.example/healthz"), request.uri)
                assertEquals(Duration.ofSeconds(5), request.timeout)
                assertTrue(request.body.isEmpty())
                return CompletableFuture.completedFuture(TransportResponse(200, ByteArray(0), "", emptyMap(), request.uri, false))
            }
        }
        val status = ToriiHealthProbe.check(executor, "https://torii.example/healthz")
        assertEquals(true, status.reachable)
        assertEquals("HTTP 200", status.message)
    }

    @Test
    fun `probe preserves server failure and asynchronous error`() = runBlocking {
        val serverError = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest) = CompletableFuture.completedFuture(
                TransportResponse(503, ByteArray(0), "", emptyMap(), request.uri, false))
        }
        assertEquals(false, ToriiHealthProbe.check(serverError, "https://torii.example").reachable)
        val transportError = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest) = CompletableFuture<TransportResponse>().also {
                it.completeExceptionally(java.io.IOException("unreachable"))
            }
        }
        val status = ToriiHealthProbe.check(transportError, "https://torii.example")
        assertEquals(false, status.reachable)
        assertTrue(status.message.contains("unreachable"))
    }
}
