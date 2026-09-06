package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayInputStream
import java.net.URI
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import org.hyperledger.iroha.sdk.client.ClientConfig
import org.hyperledger.iroha.sdk.client.HttpClientTransport
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertFailsWith
import kotlin.test.assertTrue

class HttpTransportScopeTest {
    @Test
    fun clientsCancelOnlyTheirOwnCallsOnASharedBackend() {
        val backend = DeferredBackend()
        val first = HttpTransportScope.create(backend)
        val second = HttpTransportScope.create(backend)
        val a = first.execute(request())
        val b = second.execute(request())
        first.close()
        first.close()
        assertTrue(a.isCancelled)
        assertTrue(backend.responses[0].isCancelled)
        assertFalse(b.isDone)
        assertFalse(backend.responses[1].isCancelled)
        assertEquals(0, backend.closes)
        backend.responses[1].complete(response())
        assertEquals(200, b.get(1, TimeUnit.SECONDS).statusCode)
        assertFailsWith<java.util.concurrent.ExecutionException> {
            first.execute(request()).get(1, TimeUnit.SECONDS)
        }
        assertEquals(2, backend.responses.size)
        second.close()
        assertEquals(0, backend.closes)
    }

    @Test
    fun ownerClosesItsBackendExactlyOnce() {
        val backend = DeferredBackend()
        val scope = HttpTransportScope.own(backend)
        scope.execute(request())
        scope.close()
        scope.close()
        assertEquals(1, backend.closes)
        assertTrue(backend.responses.single().isCancelled)
    }

    @Test
    fun cancellationPropagatesWithoutClosingTheClient() {
        val backend = DeferredBackend()
        HttpTransportScope.create(backend).use { scope ->
            val pending = scope.execute(request())
            assertTrue(pending.cancel(false))
            assertTrue(backend.responses.single().isCancelled)
            val next = scope.execute(request())
            backend.responses.last().complete(response())
            assertEquals(200, next.get(1, TimeUnit.SECONDS).statusCode)
        }
    }

    @Test
    fun closeDuringBackendAdmissionCancelsTheLateFuture() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val upstream = CompletableFuture<TransportResponse>()
        val backend = object : HttpTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> {
                entered.countDown()
                check(release.await(5, TimeUnit.SECONDS))
                return upstream
            }
        }
        val thread = Executors.newSingleThreadExecutor()
        try {
            val scope = HttpTransportScope.create(backend)
            val task = thread.submit<CompletableFuture<TransportResponse>> { scope.execute(request()) }
            assertTrue(entered.await(5, TimeUnit.SECONDS))
            scope.close()
            release.countDown()
            assertTrue(task.get(5, TimeUnit.SECONDS).isCancelled)
            assertTrue(upstream.isCancelled)
        } finally { release.countDown(); thread.shutdownNow() }
    }

    @Test
    fun streamOwnershipSurvivesDeliveryAndClosingOneClient() {
        val backend = StreamingBackend()
        val first = HttpTransportScope.create(backend)
        val second = HttpTransportScope.create(backend)
        val a = (first as StreamingTransportExecutor).openStream(request()).get()
        val b = (second as StreamingTransportExecutor).openStream(request()).get()
        assertEquals(0, backend.streamCloses)
        first.close()
        assertEquals(1, backend.streamCloses)
        assertFailsWith<java.io.IOException> { a.body.read() }
        assertEquals(42, b.body.read())
        b.body.close()
        assertEquals(2, backend.streamCloses)
        second.close()
        assertEquals(2, backend.streamCloses)
        assertEquals(0, backend.closes)
    }

    @Test
    fun aBufferedBackendDoesNotPretendToSupportStreaming() {
        HttpTransportScope.create(DeferredBackend()).use { scope ->
            assertFalse(scope is StreamingTransportExecutor)
        }
    }

    @Test
    fun oneFailingStreamDisposalCannotLeaveOtherStreamsOrOwnedBackendOpen() {
        var disposed = 0
        var backendClosed = false
        val backend = object : HttpTransportExecutor, StreamingTransportExecutor {
            override fun execute(request: TransportRequest) = CompletableFuture.completedFuture(response())
            override fun openStream(request: TransportRequest) = CompletableFuture.completedFuture(
                TransportStreamResponse(200, null, "OK", emptyMap(), Runnable {
                    disposed++
                    if (disposed == 1) throw IllegalStateException("injected disposal failure")
                }),
            )
            override fun close() { backendClosed = true }
        }
        val scope = HttpTransportScope.own(backend)
        val streaming = scope as StreamingTransportExecutor
        streaming.openStream(request()).get()
        streaming.openStream(request()).get()
        assertFailsWith<IllegalStateException> { scope.close() }
        assertEquals(2, disposed)
        assertTrue(backendClosed)
        scope.close()
        assertEquals(2, disposed)
    }

    @Test
    fun clientFacadeCloseCancelsPollsAndKeepsInjectedBackendAvailable() {
        val backend = DeferredBackend()
        val config = ClientConfig.builder().setBaseUri(URI("http://localhost:8080")).build()
        val first = HttpClientTransport(backend, config)
        val second = HttpClientTransport(backend, config)
        val hash = "11".repeat(32)
        val a = first.waitForTransactionStatus(hash, null)
        val b = second.waitForTransactionStatus(hash, null)
        first.close()
        assertTrue(a.isCancelled)
        assertTrue(backend.responses[0].isCancelled)
        assertFalse(b.isDone)
        assertFalse(backend.responses[1].isCancelled)
        assertEquals(0, backend.closes)
        assertFailsWith<java.util.concurrent.ExecutionException> {
            first.waitForTransactionStatus(hash, null).get(1, TimeUnit.SECONDS)
        }
        second.close()
        assertTrue(b.isCancelled)
        assertTrue(backend.responses[1].isCancelled)
        assertEquals(0, backend.closes)
    }

    private open class DeferredBackend : HttpTransportExecutor {
        val responses = ArrayList<CompletableFuture<TransportResponse>>()
        var closes = 0
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
            CompletableFuture<TransportResponse>().also { responses.add(it) }
        override fun close() { closes++ }
    }

    private class StreamingBackend : DeferredBackend(), StreamingTransportExecutor {
        var streamCloses = 0
        override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> =
            CompletableFuture.completedFuture(TransportStreamResponse(
                200, ByteArrayInputStream(byteArrayOf(42)), "OK", emptyMap(), Runnable { streamCloses++ },
            ))
    }

    private fun request(): TransportRequest = TransportRequest.builder().build()
    private fun response(): TransportResponse = TransportResponse(200, byteArrayOf(), "OK", emptyMap(), request().uri, false)
}
