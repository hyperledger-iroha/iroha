package org.hyperledger.iroha.sdk.client.stream

import java.io.ByteArrayInputStream
import java.io.InputStream
import java.net.URI
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.Executor
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertSame
import kotlin.test.assertTrue
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.HttpTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.StreamingTransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportExecutor
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportResponse
import org.hyperledger.iroha.sdk.client.transport.TransportStreamResponse

/** Stream lifetime tests use gated I/O to exercise close/response/callback races deterministically. */
class ToriiEventStreamLifecycleTest {
    @Test
    fun closingClientCancelsPendingStreamsWithoutErrorCallbacksOrClosingBorrowedBackend() {
        val backend = DeferredBackend()
        val first = client(backend)
        val second = client(backend)
        val firstListener = RecordingListener()
        val secondListener = RecordingListener()
        val a = first.open(firstListener)
        val b = second.open(secondListener)
        first.close()
        first.close()
        a.completion().get(5, TimeUnit.SECONDS)
        assertFalse(a.isOpen())
        assertTrue(backend.responses[0].isCancelled)
        assertFalse(backend.responses[1].isCancelled)
        assertFalse(b.completion().isDone)
        assertEquals(0, backend.closes)
        firstListener.assertNoCallbacks()
        second.close()
        b.completion().get(5, TimeUnit.SECONDS)
        secondListener.assertNoCallbacks()
        assertEquals(0, backend.closes)
    }

    @Test
    fun bufferedTransportCancellationAlsoCompletesQuietly() {
        val pending = CompletableFuture<TransportResponse>()
        val backend = object : TransportExecutor {
            override fun execute(request: TransportRequest) = pending
        }
        val listener = RecordingListener()
        val client = client(backend)
        val stream = client.open(listener)
        client.close()
        stream.completion().get(5, TimeUnit.SECONDS)
        assertTrue(pending.isCancelled)
        listener.assertNoCallbacks()
    }

    @Test
    fun closingFromOnOpenDisposesAttachedResponseBeforeAnyRead() {
        val body = CountingBody()
        val backend = readyBackend(body.response())
        val client = client(backend)
        val listener = object : RecordingListener() {
            override fun onOpen() { super.onOpen(); client.close() }
        }
        val stream = client.open(listener)
        stream.completion().get(5, TimeUnit.SECONDS)
        assertFalse(stream.isOpen())
        assertEquals(1, body.closes.get())
        assertEquals(0, body.reads.get())
        assertEquals(1, listener.opens.get())
        assertTrue(listener.errors.isEmpty())
        assertTrue(listener.events.isEmpty())
        assertEquals(0, listener.closed.get())
    }

    @Test
    fun listenerOnOpenFailureDisposesTheBodyAndCompletesExceptionally() {
        val body = CountingBody()
        val failure = IllegalStateException("listener failed")
        val listener = object : RecordingListener() {
            override fun onOpen() { throw failure }
        }
        client(readyBackend(body.response())).use { client ->
            val stream = client.open(listener)
            val error = assertFailsWith<ExecutionException> {
                stream.completion().get(5, TimeUnit.SECONDS)
            }
            assertSame(failure, error.cause)
            assertEquals(listOf<Throwable>(failure), listener.errors.toList())
            assertEquals(1, body.closes.get())
            assertEquals(0, body.reads.get())
        }
    }

    @Test
    fun closingWhileReadIsBlockedDiscardsLateBytesAndStopsTheReader() {
        val body = GatedBody()
        val listener = RecordingListener()
        client(readyBackend(body.response())).use { client ->
            val stream = client.open(listener)
            assertTrue(body.entered.await(5, TimeUnit.SECONDS))
            client.close()
            stream.completion().get(5, TimeUnit.SECONDS)
            assertTrue(body.disposed.await(5, TimeUnit.SECONDS))
            assertEquals(1, body.closes.get())
            assertTrue(listener.errors.isEmpty())
            assertTrue(listener.events.isEmpty())
            assertEquals(0, listener.closed.get())
        }
    }

    @Test
    fun closingDuringErrorBodyReadCancelsTheBodyWithoutReportingAnHttpFailure() {
        val body = GatedBody()
        val backend = DeferredBackend()
        val worker = Executors.newSingleThreadExecutor()
        val listener = RecordingListener()
        client(backend).use { client ->
            try {
                val stream = client.open(listener)
                val delivery = worker.submit { backend.responses.single().complete(body.response(503)) }
                assertTrue(body.entered.await(5, TimeUnit.SECONDS))
                client.close()
                delivery.get(5, TimeUnit.SECONDS)
                stream.completion().get(5, TimeUnit.SECONDS)
                assertEquals(1, body.closes.get())
                listener.assertNoCallbacks()
            } finally { body.release.countDown(); worker.shutdownNow() }
        }
    }

    @Test
    fun closingFromRequestObserverPreventsDispatch() {
        val backend = DeferredBackend()
        lateinit var client: ToriiEventStreamClient
        client = ToriiEventStreamClient.builder()
            .setBaseUri(BASE)
            .setTransportExecutor(backend)
            .addObserver(object : ClientObserver {
                override fun onRequest(request: TransportRequest) { client.close() }
            }).build()
        val listener = RecordingListener()
        client.open(listener).completion().get(5, TimeUnit.SECONDS)
        assertTrue(backend.responses.isEmpty())
        listener.assertNoCallbacks()
    }

    @Test
    fun closingWhileBackendAdmitsARequestCancelsItsLateFuture() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val pending = CompletableFuture<TransportStreamResponse>()
        val backend = object : StreamingTransportExecutor {
            override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
                error("streaming transport must not buffer")
            override fun openStream(request: TransportRequest): CompletableFuture<TransportStreamResponse> {
                entered.countDown()
                check(release.await(5, TimeUnit.SECONDS))
                return pending
            }
        }
        val worker = Executors.newSingleThreadExecutor()
        val listener = RecordingListener()
        client(backend).use { client ->
            try {
                val opening = worker.submit<ToriiEventStream> { client.open(listener) }
                assertTrue(entered.await(5, TimeUnit.SECONDS))
                client.close()
                release.countDown()
                opening.get(5, TimeUnit.SECONDS).completion().get(5, TimeUnit.SECONDS)
                assertTrue(pending.isCancelled)
                listener.assertNoCallbacks()
            } finally { release.countDown(); worker.shutdownNow() }
        }
    }

    @Test
    fun cancellingCompletionClosesTheStreamAndReleasesItsBody() {
        val body = GatedBody()
        val listener = RecordingListener()
        client(readyBackend(body.response())).use { client ->
            val stream = client.open(listener)
            assertTrue(body.entered.await(5, TimeUnit.SECONDS))
            assertTrue(stream.completion().cancel(false))
            assertFalse(stream.isOpen())
            assertTrue(body.disposed.await(5, TimeUnit.SECONDS))
            assertEquals(1, body.closes.get())
            assertTrue(listener.errors.isEmpty())
            assertTrue(listener.events.isEmpty())
        }
    }

    @Test
    fun rejectedReaderTaskReleasesTheBodyAndReportsOneFailure() {
        val body = CountingBody()
        val failure = java.util.concurrent.RejectedExecutionException("reader unavailable")
        val listener = RecordingListener()
        ToriiEventStreamClient.builder().setBaseUri(BASE)
            .setTransportExecutor(readyBackend(body.response()))
            .setReaderExecutor(Executor { throw failure }).build().use { client ->
                val stream = client.open(listener)
                val error = assertFailsWith<ExecutionException> { stream.completion().get(5, TimeUnit.SECONDS) }
                assertSame(failure, error.cause)
                assertEquals(listOf<Throwable>(failure), listener.errors.toList())
                assertEquals(1, body.closes.get())
            }
    }

    @Test
    fun readerExecutorIsBorrowedAndQueuedReaderCannotEmitAfterClosure() {
        val tasks = ArrayList<Runnable>()
        val body = CountingBody()
        val listener = RecordingListener()
        val borrowed = Executor { tasks.add(it) }
        val client = ToriiEventStreamClient.builder().setBaseUri(BASE)
            .setTransportExecutor(readyBackend(body.response()))
            .setReaderExecutor(borrowed).build()
        val stream = client.open(listener)
        assertEquals(1, tasks.size)
        client.close()
        tasks.removeAt(0).run()
        stream.completion().get(5, TimeUnit.SECONDS)
        assertEquals(0, body.reads.get())
        assertEquals(1, body.closes.get())
        assertTrue(listener.events.isEmpty())
        assertTrue(listener.errors.isEmpty())
        borrowed.execute {}
        assertEquals(1, tasks.size)
    }

    @Test
    fun closedClientRejectsNewStreamsBeforeDispatch() {
        val backend = DeferredBackend()
        val client = client(backend)
        client.close()
        assertFailsWith<IllegalStateException> { client.open(RecordingListener()) }
        assertTrue(backend.responses.isEmpty())
    }

    private open class RecordingListener : ToriiEventStreamListener {
        val opens = AtomicInteger()
        val closed = AtomicInteger()
        val events = CopyOnWriteArrayList<ServerSentEvent>()
        val errors = CopyOnWriteArrayList<Throwable>()
        override fun onOpen() { opens.incrementAndGet() }
        override fun onEvent(event: ServerSentEvent) { events.add(event) }
        override fun onClosed() { closed.incrementAndGet() }
        override fun onError(error: Throwable) { errors.add(error) }
        fun assertNoCallbacks() {
            assertEquals(0, opens.get())
            assertEquals(0, closed.get())
            assertTrue(events.isEmpty())
            assertTrue(errors.isEmpty())
        }
    }

    private class DeferredBackend : HttpTransportExecutor, StreamingTransportExecutor {
        val responses = CopyOnWriteArrayList<CompletableFuture<TransportStreamResponse>>()
        var closes = 0
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
            error("streaming transport must not buffer")
        override fun openStream(request: TransportRequest) = CompletableFuture<TransportStreamResponse>()
            .also { responses.add(it) }
        override fun close() { closes++ }
    }

    private open class CountingBody : InputStream() {
        val reads = AtomicInteger()
        val closes = AtomicInteger()
        private val payload = ByteArrayInputStream("data: late\n\n".toByteArray(Charsets.UTF_8))
        override fun read(): Int { reads.incrementAndGet(); return payload.read() }
        override fun close() { closes.incrementAndGet() }
        open fun response(status: Int = 200) = TransportStreamResponse(status, this, "", emptyMap(), null)
    }

    private class GatedBody : CountingBody() {
        val entered = CountDownLatch(1)
        val release = CountDownLatch(1)
        val disposed = CountDownLatch(1)
        override fun read(): Int {
            entered.countDown()
            check(release.await(5, TimeUnit.SECONDS))
            return super.read()
        }
        override fun close() { super.close(); disposed.countDown() }
        override fun response(status: Int) =
            TransportStreamResponse(status, this, "", emptyMap(), Runnable { release.countDown() })
    }

    private fun readyBackend(response: TransportStreamResponse) = object : StreamingTransportExecutor {
        override fun execute(request: TransportRequest): CompletableFuture<TransportResponse> =
            error("streaming transport must not buffer")
        override fun openStream(request: TransportRequest) = CompletableFuture.completedFuture(response)
    }

    private fun client(backend: TransportExecutor) = ToriiEventStreamClient.builder()
        .setBaseUri(BASE).setTransportExecutor(backend).build()

    private fun ToriiEventStreamClient.open(listener: ToriiEventStreamListener) =
        openSseStream("/v1/events/sse", null, listener)

    companion object { private val BASE = URI.create("https://example.com") }
}
