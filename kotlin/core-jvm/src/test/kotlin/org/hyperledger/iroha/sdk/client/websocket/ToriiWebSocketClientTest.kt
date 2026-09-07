package org.hyperledger.iroha.sdk.client.websocket

import java.io.IOException
import java.nio.ByteBuffer
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ExecutionException
import java.util.concurrent.atomic.AtomicInteger
import org.hyperledger.iroha.sdk.client.ClientObserver
import org.hyperledger.iroha.sdk.client.transport.TransportRequest
import org.hyperledger.iroha.sdk.client.transport.TransportWebSocket
import org.junit.jupiter.api.Test
import kotlin.test.*

/** Session lifecycle coverage independent of any connector callback scheduling. */
class ToriiWebSocketClientTest {
    @Test
    fun `early sends fail and early close cancels pending connection without error callback`() {
        val pending = CompletableFuture<TransportWebSocket>()
        val errors = AtomicInteger()
        val client = ToriiWebSocketClient.builder().setWebSocketConnector { _, _ -> pending }.build()
        val session = client.connect("events", null, object : ToriiWebSocketListener {
            override fun onError(session: ToriiWebSocketSession, error: Throwable) { errors.incrementAndGet() }
        })
        assertFalse(session.isOpen)
        assertEquals("", session.subprotocol)
        assertFailsWith<ExecutionException> { session.sendText("early").get() }
        assertFailsWith<ExecutionException> { session.sendBinary(ByteBuffer.allocate(1)).get() }
        session.close()
        session.close()
        assertTrue(pending.isCancelled)
        assertEquals(0, errors.get())
        assertFalse(session.isOpen)
    }

    @Test
    fun `failed future and connector error are delivered once`() {
        val pending = CompletableFuture<TransportWebSocket>()
        lateinit var adapter: TransportWebSocket.Listener
        val errors = AtomicInteger()
        val observations = AtomicInteger()
        val client = ToriiWebSocketClient.builder().setWebSocketConnector { _, listener ->
            adapter = listener
            pending
        }.addObserver(object : ClientObserver {
            override fun onFailure(request: TransportRequest, error: Throwable) { observations.incrementAndGet() }
        }).build()
        val session = client.connect("events", null, object : ToriiWebSocketListener {
            override fun onError(session: ToriiWebSocketSession, error: Throwable) { errors.incrementAndGet() }
        })
        val failure = IOException("handshake failed")
        adapter.onError(FakeSocket(), failure)
        pending.completeExceptionally(failure)
        assertEquals(1, errors.get())
        assertEquals(1, observations.get())
        assertFalse(session.isOpen)
        assertEquals("", session.subprotocol)
    }

    @Test
    fun `synchronous open attaches before connector returns and close does not repeat`() {
        val socket = FakeSocket()
        val opens = AtomicInteger()
        val closes = AtomicInteger()
        lateinit var adapter: TransportWebSocket.Listener
        val client = ToriiWebSocketClient.builder().setWebSocketConnector { _, listener ->
            adapter = listener
            listener.onOpen(socket)
            CompletableFuture.completedFuture(socket)
        }.build()
        val session = client.connect("events", null, object : ToriiWebSocketListener {
            override fun onOpen(session: ToriiWebSocketSession) {
                assertTrue(session.isOpen)
                session.sendText("ready").get()
                opens.incrementAndGet()
            }
            override fun onClose(session: ToriiWebSocketSession, statusCode: Int, reason: String) { closes.incrementAndGet() }
        })
        adapter.onOpen(socket)
        assertEquals(1, opens.get())
        assertEquals(1, socket.sent.get())
        adapter.onClose(socket, 1000, "done")
        adapter.onClose(socket, 1000, "done")
        assertEquals(1, closes.get())
        assertEquals(0, socket.closed.get())
        assertFalse(session.isOpen)
    }

    private class FakeSocket : TransportWebSocket {
        val sent = AtomicInteger()
        val closed = AtomicInteger()
        override fun sendText(data: String): CompletableFuture<Void> {
            sent.incrementAndGet(); return CompletableFuture.completedFuture(null)
        }
        override fun sendBinary(data: ByteBuffer): CompletableFuture<Void> = CompletableFuture.completedFuture(null)
        override fun close(statusCode: Int, reason: String): CompletableFuture<Void> {
            closed.incrementAndGet(); return CompletableFuture.completedFuture(null)
        }
        override fun isOpen(): Boolean = true
        override fun subprotocol(): String = ""
    }
}
